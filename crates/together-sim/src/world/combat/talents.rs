//! Where a hero's talents act in a run: they are given when the run is built
//! ([`World::apply_talents`]), shape every cast ([`World::talent_spec`]), scale and follow up
//! every hit ([`World::talent_hit_scale`], [`World::talent_after_hit`]), act on kills, and lean
//! the card and augment offers.
//!
//! Each hook returns at once for a hero who took no talents, so a run without talents plays
//! exactly as before.

use super::*;
use crate::spell::BURN_SECS;
use crate::talents::{
    HeroTalents, Keystone, SpellBonus, TalentWeb, EQUILIBRIUM, EQUILIBRIUM_SECS, KILL_HASTE_MAX,
    KILL_HASTE_SECS, LEECH_REACH, PAIR_MAX, POINT_BLANK_FAR, POINT_BLANK_NEAR, SEAL_HEAL,
    STREAK_ICD,
};

/// Point Blank's bonus close up, and its cost far away.
const POINT_BLANK_BONUS: f32 = 0.3;
const POINT_BLANK_COST: f32 = 0.2;
/// Steady Hand's damage.
const STEADY_HAND: f32 = 0.2;
/// Slow Burn: what hits lose, and what burns are worth after it.
const SLOW_BURN_HIT: f32 = 0.75;
const SLOW_BURN_DOT: f32 = 2.0;

/// The schools a conversion augment adds to its spell.
fn converted(a: Augment) -> &'static [Tag] {
    match a {
        Augment::Frostfire => &[Tag::Fire, Tag::Frost],
        Augment::Spellfire => &[Tag::Fire, Tag::Arcane],
        Augment::Spellfrost => &[Tag::Frost, Tag::Arcane],
        _ => &[],
    }
}

impl World {
    /// Give `player`'s hero the talent nodes `taken` (indices into `web`) as its run begins:
    /// kept only as far as they connect to its class's start, and at most the web's top number
    /// of points. Its stats rise as a tome's would, and its health fills to the new maximum.
    /// Giving talents again replaces the earlier ones. Returns how many nodes it took.
    pub fn apply_talents(&mut self, player: PlayerId, web: &TalentWeb, taken: &[usize]) -> usize {
        let Some(h) = self.hero_index(player) else {
            return 0;
        };
        let class = self.content.classes[self.heroes[h].class].key.clone();
        let kept = web.sanitize(&class, taken, web.points.max);
        let bonuses = web.bonuses(&kept);
        let hero = &mut self.heroes[h];
        let old = std::mem::take(&mut hero.talents.bonuses);
        for &(stat, v) in &old.stats {
            hero.stats.add(stat, -v);
        }
        hero.rerolls = hero.rerolls.saturating_sub(old.rerolls);
        hero.gold = hero.gold.saturating_sub(old.gold);
        for &(stat, v) in &bonuses.stats {
            hero.stats.add(stat, v);
        }
        hero.rerolls += bonuses.rerolls;
        hero.gold += bonuses.gold;
        hero.talents = HeroTalents {
            bonuses,
            nodes: kept.len() as u32,
            ..HeroTalents::default()
        };
        self.refresh(h);
        let hero = &mut self.heroes[h];
        hero.hp = hero.max_hp;
        kept.len()
    }

    /// The schools and kinds of hero `h`'s spell `o`, with the schools its conversion adds.
    fn owned_tags(&self, o: &Owned) -> Tags {
        let mut tags = self.content.abilities[o.ability].tag_set;
        for &a in &o.augments {
            for &t in converted(a) {
                tags = tags.with(t);
            }
        }
        tags
    }

    /// How many of hero `h`'s spells have `tag`.
    fn spells_with(&self, h: usize, tag: Tag) -> u32 {
        self.heroes[h]
            .abilities
            .iter()
            .filter(|o| self.owned_tags(o).has(tag))
            .count() as u32
    }

    /// Hero `h`'s talents on one of its casts: tag and hybrid bonuses, pairs, the notables that
    /// change numbers, and keystones. Called last when a cast is built, so a conversion's new
    /// school already counts.
    pub(super) fn talent_spec(&self, h: usize, s: &mut CastSpec) {
        let t = &self.heroes[h].talents;
        if t.nodes == 0 {
            return;
        }
        let t = &t.bonuses;
        let tags = s.tags;
        let get = |b: SpellBonus| t.spell_bonus(tags, b);
        let mut damage = 1.0 + get(SpellBonus::Damage);
        for &(a, b, v) in &t.pairs {
            if tags.has(a) {
                damage += v * self.spells_with(h, b).min(PAIR_MAX) as f32;
            }
            if tags.has(b) {
                damage += v * self.spells_with(h, a).min(PAIR_MAX) as f32;
            }
        }
        s.damage *= damage;
        s.crit += get(SpellBonus::Crit);
        s.crit_mult += get(SpellBonus::CritDamage);
        s.cooldown *= (1.0 - get(SpellBonus::Cooldown)).max(0.4);
        s.radius *= 1.0 + get(SpellBonus::Area);
        let longer = 1.0 + get(SpellBonus::Duration);
        s.dot_secs *= longer;
        s.ground *= longer;
        s.range *= 1.0 + get(SpellBonus::Range);
        if s.chill > 0.0 {
            s.freeze_chance += t.chill_freeze;
        }
        s.shatter += t.frozen_crit;
        s.execute += t.execute;
        if t.has(Keystone::SteadyHand) {
            s.crit = 0.0;
            s.shatter = 0.0;
            s.damage *= 1.0 + STEADY_HAND;
        }
        if t.has(Keystone::SlowBurn) {
            s.damage *= SLOW_BURN_HIT;
            s.dot_mult *= SLOW_BURN_DOT / SLOW_BURN_HIT;
        }
    }

    /// How much harder hero `h`'s hit of `spec` lands on enemy `ei` for its talents: Thermal
    /// Shock, Point Blank, and Elemental Equilibrium (which also marks the enemy).
    pub(super) fn talent_hit_scale(&mut self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        if self.heroes[h].talents.nodes == 0 {
            return 1.0;
        }
        let t = &self.heroes[h].talents.bonuses;
        let (thermal, point_blank, equilibrium) = (
            t.thermal,
            t.has(Keystone::PointBlank),
            t.has(Keystone::ElementalEquilibrium),
        );
        let e = &self.enemies[ei];
        let mut scale = 1.0;
        if thermal > 0.0
            && ((spec.tags.has(Tag::Fire) && e.status.chill > 0.0)
                || (spec.tags.has(Tag::Frost) && e.status.burning()))
        {
            scale *= 1.0 + thermal;
        }
        if point_blank {
            let d = self.heroes[h].pos.distance(e.pos);
            if d <= POINT_BLANK_NEAR {
                scale *= 1.0 + POINT_BLANK_BONUS;
            } else if d > POINT_BLANK_FAR {
                scale *= 1.0 - POINT_BLANK_COST;
            }
        }
        let schools = Tags(spec.tags.0 & Tags::of(&Tag::SCHOOLS).0);
        if equilibrium && schools.0 != 0 {
            let (id, now) = (e.id, self.elapsed);
            let marks = &mut self.heroes[h].talents.marks;
            marks.retain(|m| m.2 > now);
            match marks.iter_mut().find(|m| m.0 == id) {
                Some(m) => {
                    let brings_new = schools.0 & !m.1 .0 != 0;
                    scale *= if brings_new {
                        1.0 + EQUILIBRIUM
                    } else {
                        1.0 - EQUILIBRIUM
                    };
                    m.1 = schools;
                    m.2 = now + EQUILIBRIUM_SECS;
                }
                None => marks.push((id, schools, now + EQUILIBRIUM_SECS)),
            }
        }
        scale
    }

    /// What hero `h`'s talents do after its hit of `spec` on enemy `ei` dealt `damage`: a
    /// critical strike's burn, healing, and a crit streak's cast.
    pub(super) fn talent_after_hit(
        &mut self,
        h: usize,
        ei: usize,
        spec: &CastSpec,
        damage: f32,
        crit: bool,
    ) {
        if self.heroes[h].talents.nodes == 0 {
            return;
        }
        let player = self.heroes[h].player;
        let t = &self.heroes[h].talents.bonuses;
        let share: f32 = t
            .crit_burn
            .iter()
            .filter(|(tag, _)| spec.tags.has(*tag))
            .map(|x| x.1)
            .sum();
        let (leech, seal) = (t.leech, t.has(Keystone::SealOfLight));
        let streak_tag = t
            .crit_streak
            .iter()
            .copied()
            .find(|&tag| spec.tags.has(tag));
        if crit && share > 0.0 {
            let dps = damage * share / BURN_SECS;
            let burn = Burn {
                by: player,
                ability: spec.ability,
                dps,
                left: BURN_SECS,
                tick: 1.0,
                every: 1.0,
                contagion: spec.contagion,
                tags: spec.tags,
                leech: 0.0,
            };
            let st = &mut self.enemies[ei].status;
            match st
                .burns
                .iter_mut()
                .find(|b| b.by == player && b.ability == spec.ability)
            {
                // This hit's own burn, just laid: the crit's burn joins it.
                Some(b) if spec.burn > 0.0 => {
                    b.dps += dps;
                    b.left = b.left.max(BURN_SECS);
                }
                Some(b) => *b = burn,
                None => st.burns.push(burn),
            }
        }
        let single = matches!(spec.targeting, Targeting::Nearest | Targeting::ElitesFirst)
            && !spec.tags.has(Tag::Area);
        if leech > 0.0 && single {
            let at = self.heroes[h].pos;
            if let Some(friend) = self
                .heroes
                .iter_mut()
                .filter(|x| x.alive && x.hp < x.max_hp && x.pos.distance(at) <= LEECH_REACH)
                .min_by(|a, b| (a.hp / a.max_hp).total_cmp(&(b.hp / b.max_hp)))
            {
                friend.hp = (friend.hp + damage * leech).min(friend.max_hp);
            }
        }
        if seal {
            let hero = &mut self.heroes[h];
            if hero.alive {
                hero.hp = (hero.hp + damage * SEAL_HEAL).min(hero.max_hp);
            }
        }
        if let Some(tag) = streak_tag {
            let now = self.elapsed;
            let ht = &mut self.heroes[h].talents;
            if !crit {
                ht.streak = 0;
                return;
            }
            ht.streak = ht.streak.saturating_add(1);
            if ht.streak < 2 || now < ht.streak_ready {
                return;
            }
            ht.streak = 0;
            ht.streak_ready = now + STREAK_ICD;
            // The shortest cooldown of the tag's spells, another than this one when there is.
            let cast = self.heroes[h]
                .abilities
                .iter()
                .filter(|o| self.owned_tags(o).has(tag))
                .min_by(|a, b| {
                    let other = |o: &Owned| o.ability == spec.ability;
                    (other(a), self.spell_spec(h, a).cooldown)
                        .partial_cmp(&(other(b), self.spell_spec(h, b).cooldown))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|o| o.ability);
            if let Some(ability) = cast {
                self.triggers.push(Trigger {
                    player,
                    ability,
                    shot: Shot {
                        primary: false,
                        ..Shot::plain()
                    },
                });
            }
        }
    }

    /// A kill by hero `h`: its talents' move speed.
    pub(in crate::world) fn talent_on_kill(&mut self, h: usize) {
        let haste = self.heroes[h].talents.bonuses.kill_haste;
        if haste <= 0.0 {
            return;
        }
        let sp = &mut self.heroes[h].spell;
        // It shares Blazing Speed's haste, never taking it down.
        sp.haste = (sp.haste + haste).min(KILL_HASTE_MAX.max(sp.haste));
        sp.haste_time = sp.haste_time.max(KILL_HASTE_SECS);
        self.refresh(h);
    }

    /// Which of `options` a hand deals hero `h` next: any alike, unless its talents make some
    /// spells likelier.
    pub(in crate::world) fn offer_index(&mut self, h: usize, options: &[CardKind]) -> usize {
        let t = &self.heroes[h].talents.bonuses;
        if t.offer_weight.is_empty() {
            return self.rng.index(options.len());
        }
        let weights: Vec<f32> = options
            .iter()
            .map(|k| match k {
                CardKind::Learn(a) | CardKind::Upgrade(a, _) => {
                    t.offer_weight(self.content.abilities[*a].tag_set)
                }
                _ => 1.0,
            })
            .collect();
        let total: f32 = weights.iter().sum();
        let mut pick = self.rng.f32() * total;
        for (i, w) in weights.iter().enumerate() {
            if pick < *w {
                return i;
            }
            pick -= w;
        }
        options.len() - 1
    }

    /// How much hero `h`'s talents lean an augment screen toward `a`: 1 for no change.
    pub(super) fn talent_augment_weight(&self, h: usize, a: Augment) -> f32 {
        self.heroes[h].talents.bonuses.augment_weight(a)
    }

    /// Whether hero `h`'s talents let general augment `a` be offered on a spell cast as `spec`
    /// that already took `taken`, where its own tags would not. The other rules still hold: no
    /// augment twice, one conversion, one cone shape.
    pub(super) fn talent_unlocks(
        &self,
        h: usize,
        a: Augment,
        spec: &CastSpec,
        taken: &[Augment],
    ) -> bool {
        a.capstone_of().is_none()
            && !taken.contains(&a)
            && !(a.is_conversion() && taken.iter().any(|x| x.is_conversion()))
            && !(a.is_cone_shape() && taken.iter().any(|x| x.is_cone_shape()))
            && self.heroes[h].talents.bonuses.unlocks(a, spec.tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentSources;
    use crate::talents::{TalentEffect, TalentWeb};

    fn shipped() -> Arc<Content> {
        Arc::new(
            Content::from_ron(ContentSources {
                classes: include_str!("../../../../../content/classes.ron"),
                abilities: include_str!("../../../../../content/abilities.ron"),
                enemies: include_str!("../../../../../content/enemies.ron"),
                zones: include_str!("../../../../../content/zones.ron"),
            })
            .expect("shipped content is valid"),
        )
    }

    /// Give hero 0 exactly these spells, each at `rank` with these augments.
    fn spells(w: &mut World, list: &[(&str, u8, &[Augment])]) {
        w.heroes[0].abilities = list
            .iter()
            .map(|&(key, rank, augments)| Owned {
                level: rank,
                augments: augments.to_vec(),
                cd: 0.0,
                ..Owned::new(w.content.ability(key).unwrap(), 0.0)
            })
            .collect();
    }

    /// An enemy with `hp`, `dx` and `dy` yards from hero 0; its index.
    fn foe_at(w: &mut World, dx: f32, dy: f32, hp: f32) -> usize {
        let pos = w.heroes[0].pos + V2::new(dx, dy);
        let id = w.spawn_enemy(0, pos, pos, false);
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        w.enemies[ei].hp = hp;
        w.enemies[ei].max_hp = hp;
        ei
    }

    /// A web whose mage start links straight to one node per effect, so any of them can be
    /// taken alone.
    fn web(effects: &[TalentEffect]) -> TalentWeb {
        let mut text = String::from(
            "(points: (max: 40, first: 100, step: 50), nodes: [
            (id: \"start\", name: \"Mage\", kind: Start, class: Some(\"mage\"), pos: (0, 0), links: [",
        );
        for i in 0..effects.len() {
            text += &format!("\"n{i}\",");
        }
        text += "]),";
        for (i, e) in effects.iter().enumerate() {
            text += &format!(
                "(id: \"n{i}\", name: \"N{i}\", kind: Notable, pos: ({i}, 0), effects: [{}]),",
                ron::to_string(e).unwrap()
            );
        }
        text += "])";
        TalentWeb::from_ron(&text).unwrap()
    }

    fn mage_with(effects: &[TalentEffect]) -> World {
        let mut w = World::new(shipped(), "elwynn", 7).unwrap();
        w.add_hero(0, "mage").unwrap();
        let web = web(effects);
        let all: Vec<usize> = (1..web.nodes.len()).collect();
        assert_eq!(w.apply_talents(0, &web, &all), effects.len());
        w
    }

    fn spec_of(w: &World, key: &str) -> CastSpec {
        let ability = w.content().ability(key).unwrap();
        let o = w.heroes[0]
            .abilities
            .iter()
            .find(|o| o.ability == ability)
            .unwrap()
            .clone();
        w.spell_spec(0, &o)
    }

    #[test]
    fn stats_rise_and_health_fills_and_giving_again_replaces() {
        let mut w = World::new(shipped(), "elwynn", 7).unwrap();
        w.add_hero(0, "mage").unwrap();
        let base = w.heroes[0].max_hp;
        let rerolls = w.heroes[0].rerolls;
        let web = web(&[
            TalentEffect::Stat(Stat::Health, 0.1),
            TalentEffect::Rerolls(1),
            TalentEffect::Gold(25),
        ]);
        w.apply_talents(0, &web, &[1, 2, 3]);
        let hero = &w.heroes[0];
        assert!((hero.max_hp - base * 1.1).abs() < 1e-3);
        assert_eq!(hero.hp, hero.max_hp);
        assert_eq!((hero.rerolls, hero.gold), (rerolls + 1, 25));
        w.apply_talents(0, &web, &[]);
        let hero = &w.heroes[0];
        assert!((hero.max_hp - base).abs() < 1e-3, "taken back, not doubled");
        assert_eq!((hero.rerolls, hero.gold), (rerolls, 0));
        assert_eq!(hero.talents.nodes, 0);
    }

    #[test]
    fn nodes_out_of_reach_or_past_the_points_are_not_given() {
        let mut w = World::new(shipped(), "elwynn", 7).unwrap();
        w.add_hero(0, "warrior").unwrap();
        let web = web(&[TalentEffect::Stat(Stat::Damage, 0.1)]);
        assert_eq!(
            w.apply_talents(0, &web, &[1]),
            0,
            "the mage's web, not the warrior's"
        );
        assert_eq!(w.apply_talents(9, &web, &[1]), 0, "no such hero");
    }

    #[test]
    fn tag_bonuses_reach_only_their_spells_and_follow_a_conversion() {
        let mut w = mage_with(&[TalentEffect::Tagged(Tag::Fire, SpellBonus::Damage, 0.5)]);
        spells(&mut w, &[("fireball", 1, &[]), ("frostbolt", 1, &[])]);
        let plain = {
            let mut bare = World::new(shipped(), "elwynn", 7).unwrap();
            bare.add_hero(0, "mage").unwrap();
            spells(&mut bare, &[("fireball", 1, &[]), ("frostbolt", 1, &[])]);
            (
                spec_of(&bare, "fireball").damage,
                spec_of(&bare, "frostbolt").damage,
            )
        };
        assert!((spec_of(&w, "fireball").damage - plain.0 * 1.5).abs() < 1e-3);
        assert!((spec_of(&w, "frostbolt").damage - plain.1).abs() < 1e-3);
        spells(&mut w, &[("frostbolt", 5, &[Augment::Frostfire])]);
        assert!(
            spec_of(&w, "frostbolt").damage > plain.1 * 1.4,
            "Frostfire makes Frostbolt a Fire spell too"
        );
    }

    /// The benchmark build of `docs/DESIGN-IDEAS.md` section D: slows can Freeze, crits on the
    /// Frozen, Fire crits Ignite, and Frostfire spells hit harder.
    #[test]
    fn the_frostfire_benchmark_fits_together() {
        let mut w = mage_with(&[
            TalentEffect::ChillFreeze(0.15),
            TalentEffect::FrozenCrit(0.25),
            TalentEffect::CritBurn(Tag::Fire, 0.4),
            TalentEffect::Hybrid(Tag::Fire, Tag::Frost, SpellBonus::Damage, 0.1),
        ]);
        // Blast Wave slows, so it can now Freeze; augmented to Frostfire, it is Frost too.
        spells(&mut w, &[("blast_wave", 5, &[Augment::Frostfire])]);
        let wave = spec_of(&w, "blast_wave");
        assert!(wave.freeze_chance >= 0.15);
        assert!(wave.tags.has(Tag::Frost));
        // Pyroblast with the same conversion crits the Frozen.
        spells(&mut w, &[("pyroblast", 5, &[Augment::Frostfire])]);
        let pyro = spec_of(&w, "pyroblast");
        assert!(pyro.shatter >= 0.25);
        // Fireball as Frostfire Bolt slows, so its hits can Freeze too.
        spells(&mut w, &[("fireball", 5, &[Augment::Frostfire])]);
        assert!(spec_of(&w, "fireball").freeze_chance >= 0.15);
        // A Fire crit leaves a burn of 40% of the hit.
        let ei = foe_at(&mut w, 3.0, 0.0, 1e6);
        let mut spec = spec_of(&w, "fireball");
        spec.burn = 0.0;
        w.talent_after_hit(0, ei, &spec, 100.0, true);
        let left = w.enemies[ei].status.burn_left();
        assert!((left - 40.0).abs() < 1e-3, "40% of the hit: {left}");
        w.talent_after_hit(0, ei, &spec, 100.0, false);
        assert!(
            (w.enemies[ei].status.burn_left() - 40.0).abs() < 1e-3,
            "no crit, no burn"
        );
    }

    #[test]
    fn a_pair_rewards_knowing_both_schools() {
        let mut w = mage_with(&[TalentEffect::Pair(Tag::Fire, Tag::Frost, 0.1)]);
        spells(&mut w, &[("fireball", 1, &[])]);
        let alone = spec_of(&w, "fireball").damage;
        spells(
            &mut w,
            &[
                ("fireball", 1, &[]),
                ("frostbolt", 1, &[]),
                ("blizzard", 1, &[]),
            ],
        );
        let mixed = spec_of(&w, "fireball").damage;
        assert!(
            (mixed / alone - 1.2 / 1.0).abs() < 1e-3,
            "two Frost spells: +20%"
        );
    }

    #[test]
    fn steady_hand_never_crits_and_slow_burn_trades_hits_for_burns() {
        let w = mage_with(&[TalentEffect::Keystone(Keystone::SteadyHand)]);
        let s = spec_of(&w, "frostbolt");
        assert_eq!(s.crit, 0.0);
        let w = mage_with(&[TalentEffect::Keystone(Keystone::SlowBurn)]);
        let s = spec_of(&w, "frostbolt");
        assert!(
            (s.dot_mult * s.damage / (spec_of(&mage_with(&[]), "frostbolt").damage) - 2.0).abs()
                < 1e-3
        );
    }

    #[test]
    fn elemental_equilibrium_rewards_changing_schools() {
        let mut w = mage_with(&[TalentEffect::Keystone(Keystone::ElementalEquilibrium)]);
        spells(&mut w, &[("fireball", 1, &[]), ("frostbolt", 1, &[])]);
        let ei = foe_at(&mut w, 3.0, 0.0, 1e6);
        let fire = spec_of(&w, "fireball");
        let frost = spec_of(&w, "frostbolt");
        assert_eq!(
            w.talent_hit_scale(0, ei, &fire),
            1.0,
            "the first hit only marks"
        );
        assert_eq!(w.talent_hit_scale(0, ei, &frost), 1.25, "a new school");
        assert_eq!(
            w.talent_hit_scale(0, ei, &frost),
            0.75,
            "the same school again"
        );
    }

    #[test]
    fn point_blank_hits_close_harder_and_far_softer() {
        let mut w = mage_with(&[TalentEffect::Keystone(Keystone::PointBlank)]);
        let a = foe_at(&mut w, 3.0, 0.0, 1e6);
        let b = foe_at(&mut w, 25.0, 0.0, 1e6);
        let s = spec_of(&w, "frostbolt");
        assert!((w.talent_hit_scale(0, a, &s) - 1.3).abs() < 1e-6);
        assert!((w.talent_hit_scale(0, b, &s) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn a_holy_streak_casts_a_holy_spell() {
        let mut w = World::new(shipped(), "elwynn", 7).unwrap();
        w.add_hero(0, "priest").unwrap();
        let text = "(points: (max: 40, first: 100, step: 50), nodes: [
            (id: \"start\", name: \"Priest\", kind: Start, class: Some(\"priest\"), pos: (0, 0), links: [\"n\"]),
            (id: \"n\", name: \"Divine Fury\", kind: Notable, pos: (1, 0), effects: [CritStreak(Holy)]),
        ])";
        let web = TalentWeb::from_ron(text).unwrap();
        w.apply_talents(0, &web, &[1]);
        spells(&mut w, &[("smite", 1, &[]), ("holy_nova", 1, &[])]);
        let ei = foe_at(&mut w, 3.0, 0.0, 1e6);
        let smite = spec_of(&w, "smite");
        w.talent_after_hit(0, ei, &smite, 10.0, true);
        assert!(w.triggers.is_empty(), "one crit is not a streak");
        w.talent_after_hit(0, ei, &smite, 10.0, true);
        assert_eq!(w.triggers.len(), 1);
        assert_eq!(
            w.triggers[0].ability,
            w.content().ability("holy_nova").unwrap(),
            "another Holy spell than the one that crit"
        );
        w.talent_after_hit(0, ei, &smite, 10.0, true);
        w.talent_after_hit(0, ei, &smite, 10.0, true);
        assert_eq!(w.triggers.len(), 1, "not again within its cooldown");
    }

    #[test]
    fn a_leech_heals_the_most_wounded_friend_near() {
        let mut w = mage_with(&[TalentEffect::Leech(0.1)]);
        w.add_hero(1, "warrior").unwrap();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.9;
        w.heroes[1].hp = w.heroes[1].max_hp * 0.5;
        let before = w.heroes[1].hp;
        let ei = foe_at(&mut w, 3.0, 0.0, 1e6);
        let bolt = spec_of(&w, "frostbolt");
        w.talent_after_hit(0, ei, &bolt, 50.0, false);
        assert!((w.heroes[1].hp - before - 5.0).abs() < 1e-3);
    }

    #[test]
    fn offers_lean_toward_a_weighted_school() {
        let mut w = mage_with(&[TalentEffect::OfferWeight(Tag::Fire, 9.0)]);
        let fire = w.content().ability("fireball").unwrap();
        let frost = w.content().ability("frost_nova").unwrap();
        let options = [CardKind::Learn(frost), CardKind::Learn(fire)];
        let fires = (0..400).filter(|_| w.offer_index(0, &options) == 1).count();
        assert!(fires > 320, "ten to one: {fires} of 400");
    }

    #[test]
    fn an_unlock_offers_an_augment_its_tags_would_not() {
        let mut w = mage_with(&[TalentEffect::Unlock(Augment::Shatter, vec![Tag::Slow])]);
        spells(&mut w, &[("frostbolt", 5, &[])]);
        let bolt = spec_of(&w, "frostbolt");
        assert!(!Augment::Shatter.fits(&bolt, &[]), "Shatter needs Freeze");
        assert!(w.talent_unlocks(0, Augment::Shatter, &bolt, &[]));
        assert!(!w.talent_unlocks(0, Augment::Shatter, &bolt, &[Augment::Shatter]));
        let mut seen = false;
        for _ in 0..60 {
            let ability = w.content().ability("frostbolt").unwrap();
            let offer = w.augment_offer(0, ability).unwrap();
            seen |= offer
                .cards
                .iter()
                .any(|c| c.kind == CardKind::Augment(ability, Augment::Shatter));
        }
        assert!(seen, "Shatter turns up on Frostbolt's augment screen");
    }

    #[test]
    fn a_kill_hastens_the_hero() {
        let mut w = mage_with(&[TalentEffect::KillHaste(0.05)]);
        let speed = w.heroes[0].speed;
        for _ in 0..10 {
            w.talent_on_kill(0);
        }
        assert!((w.heroes[0].speed / speed - 1.2).abs() < 1e-3, "up to +20%");
    }
}
