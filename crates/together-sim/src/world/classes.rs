//! What the other classes bring to casting, beside the mage's mechanics in `combat.rs`: Ward
//! spells and their charges (heal, block and thorn charges, with overflow and retaliation, design
//! rules 11b and 11c), Buffs and Auras, Fear, Root, Weakened, Marks and Sunder, lifesteal, combo
//! points, chains, summons and totems, traps, the most-wounded targeting, and the defensive button
//! every class has on F.
//!
//! The hooks here are called from `combat.rs` and `world.rs` where a cast is built, fired, lands,
//! kills, or a blow reaches a hero; each class's capstones live in a file of their own
//! (`warrior.rs` and the rest), called from these hooks.

use serde::{Deserialize, Serialize};

use super::combat::{Crit, Death, Shot};
use super::*;
use crate::content::WardKind;
use crate::spell::{
    control_secs, BuffFx, Tag, Targeting, CHAIN_REACH, COMBO_POWER, MAX_COMBO, SUNDER_EACH,
    SUNDER_MAX, SUNDER_SECS, WEAKEN,
};

/// How far a heal charge reaches an ally, and an Aura its allies.
pub const ALLY_REACH: f32 = 30.0;
pub const AURA_REACH: f32 = 20.0;
/// Seconds an unsprung trap waits before it fades, and the share of its burst each later tick
/// deals.
const TRAP_WAIT: f32 = 30.0;
pub(super) const TRAP_TICK: f32 = 0.25;
/// Seconds a Renewing heal runs over.
const RENEW_SECS: f32 = 6.0;
/// How far Sweeping's extra hit and Thunderstrike's arc reach from the target.
const SWEEP_REACH: f32 = 3.0;
const ARC_REACH: f32 = 8.0;

/// A spell's effects on an enemy from the other classes: Fear, Root, Weakened, a Mark, Sunder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusFx {
    /// Seconds it flees from `fear_from`.
    pub fear: f32,
    pub fear_from: V2,
    /// Seconds it cannot move.
    pub root: f32,
    /// Seconds it deals `weaken_cut` less ([`WEAKEN`] for most spells).
    pub weaken: f32,
    pub weaken_cut: f32,
    /// Extra damage it takes from every source while `mark_secs` lasts.
    pub mark: f32,
    pub mark_secs: f32,
    /// Sunder stacks, and seconds before they go.
    pub sunder: u8,
    pub sunder_secs: f32,
    /// Seconds a hero's Smoke Bomb hides the heroes from it (it mills about).
    pub lost: f32,
    /// A rogue's Blind left it open: that hero's next hit on it deals double (Sap).
    pub sapped_by: Option<PlayerId>,
    /// The priest whose Hex of Weakness is on it.
    pub hexed_by: Option<PlayerId>,
}

impl StatusFx {
    /// How much more it takes from every hit: its Mark and its Sunder.
    pub fn exposure(&self) -> f32 {
        let mark = if self.mark_secs > 0.0 { self.mark } else { 0.0 };
        (1.0 + mark) * (1.0 + SUNDER_EACH * f32::from(self.sunder))
    }
}

/// A Ward spell's bank of charges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    pub ability: usize,
    pub kind: WardKind,
    pub charges: u8,
}

/// A Buff at work on a hero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveBuff {
    pub ability: usize,
    pub by: PlayerId,
    pub fx: BuffFx,
    pub left: f32,
}

/// A capstone's timed bonus on a hero (Rampage's damage, Nimble's speed), keyed by the augment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boon {
    pub key: crate::augments::Augment,
    pub value: f32,
    pub left: f32,
}

/// The other classes' spell state on a hero.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroFx {
    /// Seconds until the defensive button is ready, and since it was last used.
    pub defense_cd: f32,
    pub since_defense: f32,
    /// Ward banks, one per Ward spell.
    pub banks: Vec<Bank>,
    pub buffs: Vec<ActiveBuff>,
    /// Seconds the horde has lost this hero.
    pub hidden: f32,
    /// Rogue combo points.
    pub combo: u8,
    /// Heals over time (Renewing): health a second and seconds left.
    pub hots: Vec<(f32, f32)>,
    pub boons: Vec<Boon>,
    /// The enemy a focusing capstone last hit (Searing Light).
    pub focus: Option<u32>,
    /// Counters capstones keep (every 3rd swing, blows taken), keyed by the augment.
    pub counts: Vec<(crate::augments::Augment, u32)>,
}

impl HeroFx {
    /// Extra move speed from buffs and boons.
    pub fn haste(&self) -> f32 {
        self.buffs.iter().map(|b| b.fx.haste).sum::<f32>()
            + self.boon(crate::augments::Augment::Nimble)
            + self.boon(crate::augments::Augment::BodyAndSoul)
    }

    /// The share of blows its buffs take away (the strongest counts).
    pub fn guard(&self) -> f32 {
        self.buffs.iter().map(|b| b.fx.guard).fold(0.0, f32::max)
    }

    /// How much faster its cooldowns run (0.3 is 30% faster).
    pub fn cd_rate(&self) -> f32 {
        self.buffs.iter().map(|b| b.fx.cd_rate).sum()
    }

    /// Extra damage from its buffs.
    pub fn power(&self) -> f32 {
        self.buffs.iter().map(|b| b.fx.power).sum()
    }

    pub fn charges(&self, ability: usize) -> u8 {
        self.banks
            .iter()
            .find(|b| b.ability == ability)
            .map_or(0, |b| b.charges)
    }

    /// A capstone's timed bonus, 0 when it has run out.
    pub fn boon(&self, key: crate::augments::Augment) -> f32 {
        self.boons
            .iter()
            .find(|b| b.key == key && b.left > 0.0)
            .map_or(0.0, |b| b.value)
    }

    /// Raise a capstone's bonus by `add`, up to `max`, and give it `secs` more.
    pub fn add_boon(&mut self, key: crate::augments::Augment, add: f32, max: f32, secs: f32) {
        match self.boons.iter_mut().find(|b| b.key == key) {
            Some(b) => {
                b.value = if b.left > 0.0 { b.value + add } else { add }.min(max);
                b.left = secs;
            }
            None => self.boons.push(Boon {
                key,
                value: add.min(max),
                left: secs,
            }),
        }
    }

    /// Count one more for `key`; the new count.
    pub fn bump(&mut self, key: crate::augments::Augment) -> u32 {
        match self.counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                self.counts.push((key, 1));
                1
            }
        }
    }

    /// Set the count for `key` back to 0.
    pub fn reset(&mut self, key: crate::augments::Augment) {
        if let Some((_, n)) = self.counts.iter_mut().find(|(k, _)| *k == key) {
            *n = 0;
        }
    }
}

impl World {
    /// Hero `h`'s spell `ability` as it owns it, or at rank 1 if it does not.
    pub(super) fn owned_or_new(&self, h: usize, ability: usize) -> Owned {
        self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == ability)
            .cloned()
            .unwrap_or_else(|| Owned::new(ability, 0.0))
    }

    /// The cast of hero `h`'s spell `key` (rank 1 if it does not know it); `None` if the content
    /// has no such spell.
    pub(super) fn spec_of(&self, h: usize, key: &str) -> Option<CastSpec> {
        let ability = self.content.ability(key)?;
        Some(self.spell_spec(h, &self.owned_or_new(h, ability)))
    }

    /// Hero `h`'s spell `key` (rank 1 if it does not know it) struck at enemy `id` alone, as a
    /// capstone applying it (Blood and Thunder's Rend).
    pub(super) fn cast_on(&mut self, h: usize, key: &str, id: u32) {
        let Some(spec) = self.spec_of(h, key) else {
            return;
        };
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        self.strike(player, &spec, id, 1.0, pos, Crit::Roll);
    }

    /// Put a buff with `fx` from hero `by`'s spell `ability` on hero `i`, in place of the same
    /// one.
    pub(super) fn grant(&mut self, i: usize, by: PlayerId, ability: usize, fx: BuffFx) {
        let hero = &mut self.heroes[i];
        hero.spell
            .fx
            .buffs
            .retain(|b| b.ability != ability || b.fx != fx);
        hero.spell.fx.buffs.push(ActiveBuff {
            ability,
            by,
            fx,
            left: fx.secs,
        });
        self.refresh(i);
    }

    /// The `count` most wounded enemies within `range` of `from` (least health left as a share
    /// of their maximum), never `exclude`; the most wounded first.
    pub(super) fn pick_weakest(
        &self,
        from: V2,
        range: f32,
        count: usize,
        exclude: Option<u32>,
    ) -> Vec<(u32, f32)> {
        let mut near: Vec<(u32, f32, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && Some(e.id) != exclude)
            .map(|e| (e.id, from.distance(e.pos), e.hp / e.max_hp.max(1.0)))
            .filter(|&(_, d, _)| d <= range)
            .collect();
        near.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.1.total_cmp(&b.1)));
        near.truncate(count);
        near.into_iter().map(|(id, d, _)| (id, d)).collect()
    }

    /// The other classes' part of building a cast: a heal grows with every damage bonus the spell
    /// has (healing power is its damage stat, 11c), thorns come from its damage, buffs add their
    /// damage, and the class capstones that change numbers.
    pub(super) fn class_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let def = &self.content.abilities[owned.ability];
        if def.damage > 0.0 {
            s.fx.heal = def.heal * s.damage / def.damage;
        }
        s.fx.buff.thorns = def.thorns * s.damage;
        s.damage *= 1.0 + self.heroes[h].spell.fx.power();
        self.warrior_spec(h, owned, s);
        self.rogue_spec(h, owned, s);
        self.priest_spec(h, owned, s);
    }

    /// The share of each tick of `by`'s damage over time from `spec` that heals `by`, beside the
    /// spell's own lifesteal (Blood Frenzy's bleeds).
    pub(super) fn dot_leech(&self, by: PlayerId, spec: &CastSpec) -> f32 {
        let Some(h) = self.hero_ix(by) else {
            return 0.0;
        };
        self.warrior_dot_leech(h, spec)
    }

    /// How fast hero `h`'s spell `owned` comes back: 1 is its own pace.
    pub(super) fn cast_rate(&self, h: usize, owned: &Owned) -> f32 {
        let fx = &self.heroes[h].spell.fx;
        let mut rate = 1.0 + fx.cd_rate();
        rate *= self.rogue_rate(h, owned);
        rate
    }

    /// Just before hero `h` fires `owned` as `spec` on its own cooldown: a Finisher counts its
    /// combo points, and capstones that act on every few casts.
    pub(super) fn before_cast(&mut self, h: usize, owned: &Owned, spec: &mut CastSpec) {
        if spec.tags.has(Tag::Finisher) {
            let k = 1.0 + COMBO_POWER * f32::from(self.heroes[h].spell.fx.combo);
            spec.damage *= k;
            spec.stun *= k;
            spec.fx.buff.secs *= k;
        }
        self.warrior_before(h, owned, spec);
        self.rogue_before(h, owned, spec);
        self.priest_before(h, owned, spec);
    }

    /// Hero `h` fired `owned` as `spec`: a Finisher spends its combo points, a builder gives
    /// them.
    pub(super) fn after_cast(&mut self, h: usize, owned: &Owned, spec: &CastSpec) {
        if spec.tags.has(Tag::Finisher) {
            let spent = std::mem::take(&mut self.heroes[h].spell.fx.combo);
            self.rogue_spent(h, owned, spec, spent);
        }
        if spec.fx.combo > 0 {
            self.give_combo(h, spec.fx.combo);
        }
        self.warrior_after(h, owned, spec);
        self.rogue_after(h, owned, spec);
        self.priest_after(h, owned, spec);
    }

    /// `n` more combo points for hero `h`, up to [`MAX_COMBO`].
    pub(super) fn give_combo(&mut self, h: usize, n: u8) {
        let fx = &mut self.heroes[h].spell.fx;
        fx.combo = (fx.combo + n).min(MAX_COMBO);
    }

    /// A Ward spell's cast: one more charge in its bank, or, with the bank full, its overflow.
    /// False when the bank is full and nothing is in reach of the overflow.
    pub(super) fn bank_charge(&mut self, h: usize, spec: &CastSpec) -> bool {
        let kind = spec.fx.ward.unwrap_or(WardKind::Block);
        let cap = spec.charges_held();
        let player = self.heroes[h].player;
        let fx = &mut self.heroes[h].spell.fx;
        let i = match fx.banks.iter().position(|b| b.ability == spec.ability) {
            Some(i) => i,
            None => {
                fx.banks.push(Bank {
                    ability: spec.ability,
                    kind,
                    charges: 0,
                });
                fx.banks.len() - 1
            }
        };
        if fx.banks[i].charges < cap {
            fx.banks[i].charges += 1;
            self.events.push(Event::Cast {
                caster: player,
                ability: spec.ability,
                visual: self.content.abilities[spec.ability].visual,
                targets: vec![Target::Hero(player)],
                dest: None,
                cooldown: spec.cooldown,
            });
            return true;
        }
        self.overflow(h, spec)
    }

    /// One more charge in hero `h`'s bank for the Ward spell `ability` (a capstone banking one),
    /// up to its cap.
    pub(super) fn add_charge(&mut self, h: usize, ability: usize) {
        let spec = self.spell_spec(h, &self.owned_or_new(h, ability));
        let cap = spec.charges_held();
        let kind = spec.fx.ward.unwrap_or(WardKind::Block);
        let fx = &mut self.heroes[h].spell.fx;
        match fx.banks.iter_mut().find(|b| b.ability == ability) {
            Some(b) => b.charges = (b.charges + 1).min(cap),
            None => fx.banks.push(Bank {
                ability,
                kind,
                charges: 1,
            }),
        }
    }

    /// A full Ward's cast strikes instead (11c): the nearest enemies in its range, bosses and
    /// elites first.
    pub(super) fn overflow(&mut self, h: usize, spec: &CastSpec) -> bool {
        let pos = self.heroes[h].pos;
        let targets = self.pick_targets(
            pos,
            spec.range.max(8.0),
            spec.fx.overflow.max(1) as usize,
            true,
            None,
        );
        if targets.is_empty() {
            return false;
        }
        let player = self.heroes[h].player;
        self.events.push(Event::Cast {
            caster: player,
            ability: spec.ability,
            visual: self.content.abilities[spec.ability].visual,
            targets: targets.iter().map(|&(id, _)| Target::Enemy(id)).collect(),
            dest: None,
            cooldown: spec.cooldown,
        });
        let scale = if spec.fx.overflow > 1 { 1.5 } else { 1.0 };
        for (id, _) in targets {
            self.strike(player, spec, id, scale, pos, Crit::Roll);
        }
        self.warrior_overflow(h, spec);
        true
    }

    /// A Buff spell's cast: its effect on hero `h`, and on every ally within reach for an Aura.
    pub(super) fn buff(&mut self, h: usize, spec: &CastSpec) -> bool {
        let player = self.heroes[h].player;
        let pos = self.heroes[h].pos;
        let fx = spec.fx.buff;
        let aura = spec.tags.has(Tag::Aura);
        let who: Vec<usize> = (0..self.heroes.len())
            .filter(|&i| {
                self.heroes[i].alive
                    && (i == h || (aura && self.heroes[i].pos.distance(pos) <= AURA_REACH))
            })
            .collect();
        for i in who {
            let hero = &mut self.heroes[i];
            hero.spell.fx.buffs.retain(|b| b.ability != spec.ability);
            if fx.secs > 0.0 {
                hero.spell.fx.buffs.push(ActiveBuff {
                    ability: spec.ability,
                    by: player,
                    fx,
                    left: fx.secs,
                });
            }
            if i == h && fx.hide > 0.0 {
                hero.spell.fx.hidden = hero.spell.fx.hidden.max(fx.hide);
            }
            self.refresh(i);
        }
        self.events.push(Event::Cast {
            caster: player,
            ability: spec.ability,
            visual: self.content.abilities[spec.ability].visual,
            targets: vec![Target::Hero(player)],
            dest: None,
            cooldown: spec.cooldown,
        });
        true
    }

    /// Heal hero `t` by `amount` from hero `h`'s spell `spec`: Mending Link heals the heroes
    /// around it too, Renewing adds a heal over time. What it healed.
    pub(super) fn heal_hero(&mut self, t: usize, amount: f32, spec: Option<&CastSpec>) -> f32 {
        let hero = &mut self.heroes[t];
        if !hero.alive || amount <= 0.0 {
            return 0.0;
        }
        let before = hero.hp;
        hero.hp = (hero.hp + amount).min(hero.max_hp);
        let healed = hero.hp - before;
        let Some(spec) = spec else {
            return healed;
        };
        if spec.fx.renewing > 0.0 {
            let per = amount * spec.fx.renewing / RENEW_SECS;
            self.heroes[t].spell.fx.hots.push((per, RENEW_SECS));
        }
        if spec.fx.mending > 0.0 {
            let at = self.heroes[t].pos;
            for o in 0..self.heroes.len() {
                if o != t && self.heroes[o].alive && self.heroes[o].pos.distance(at) <= 10.0 {
                    let hero = &mut self.heroes[o];
                    hero.hp = (hero.hp + amount * spec.fx.mending).min(hero.max_hp);
                }
            }
        }
        healed
    }

    /// Hero `h` heals itself for `amount` of lifesteal; with Sanguine, what goes past full health
    /// becomes a shield.
    pub(super) fn leech(&mut self, h: usize, amount: f32, sanguine: f32) {
        let hero = &mut self.heroes[h];
        if !hero.alive || amount <= 0.0 {
            return;
        }
        let room = hero.max_hp - hero.hp;
        hero.hp = (hero.hp + amount).min(hero.max_hp);
        let spare = amount - room;
        if sanguine > 0.0 && spare > 0.0 {
            let sp = &mut hero.spell;
            sp.shield = (sp.shield + spare)
                .min(hero.max_hp * sanguine)
                .max(sp.shield);
            sp.shield_time = sp.shield_time.max(10.0);
        }
    }

    /// How much harder a hit of `spec` from hero `h` lands on enemy `ei`: its Mark and Sunder (any
    /// source), and the spell's bonuses against what holds the enemy and against elites.
    pub(super) fn hit_scale(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let e = &self.enemies[ei];
        let st = &e.status;
        let mut m = st.fx.exposure();
        let stunned = st.stun > 0.0;
        let feared = st.fx.fear > 0.0;
        if stunned || feared || st.freeze > 0.0 {
            m *= 1.0 + spec.fx.vs_held;
        }
        if feared {
            m *= 1.0 + spec.fx.vs_feared;
        }
        if st.fx.root > 0.0 {
            m *= 1.0 + spec.fx.vs_rooted;
        }
        if stunned {
            m *= 1.0 + spec.fx.vs_stunned;
        }
        if e.elite || e.boss {
            m *= 1.0 + spec.fx.vs_elite;
        }
        let player = self.heroes[h].player;
        if spec.tags.has(Tag::Physical)
            && st.has_dot(player, Tag::Bleed)
            && self.knows(h, crate::augments::Augment::Hemorrhage)
        {
            m *= 1.2;
        }
        m * self.warrior_scale(h, ei, spec) * self.rogue_scale(h, ei, spec)
    }

    /// A hit of `spec` by hero `h` landed on enemy `id` (index `ei`) for `damage`: lifesteal,
    /// Sweeping's and Thunderstrike's extra hits, and the class capstones that act on a hit.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn on_strike(
        &mut self,
        h: usize,
        ei: usize,
        id: u32,
        spec: &CastSpec,
        damage: f32,
        crit: bool,
        scale: f32,
    ) {
        if spec.fx.leech > 0.0 {
            self.leech(h, damage * spec.fx.leech, spec.fx.sanguine);
        }
        let at = self.enemies[ei].pos;
        let player = self.heroes[h].player;
        let crusader = crate::augments::Augment::Crusader;
        if self.spell_has(player, spec.ability, crusader) && self.heroes[h].spell.ready(crusader) {
            self.heroes[h].spell.spend(crusader, 1.0);
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + hero.max_hp * 0.02).min(hero.max_hp);
        }
        for (share, reach) in [(spec.fx.sweep, SWEEP_REACH), (spec.fx.arc, ARC_REACH)] {
            if share <= 0.0 {
                continue;
            }
            let next = self
                .enemies
                .iter()
                .filter(|e| e.id != id && e.hp > 0.0 && e.pos.distance(at) <= reach)
                .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
                .map(|e| e.id);
            if let Some(next) = next {
                let mut plain = spec.clone();
                plain.fx.sweep = 0.0;
                plain.fx.arc = 0.0;
                plain.fx.jumps = 0;
                self.strike(player, &plain, next, scale * share, at, Crit::Never);
            }
        }
        self.warrior_strike(h, id, spec, damage, crit, scale);
        self.rogue_strike(h, id, spec, damage, crit, scale);
        self.priest_strike(h, id, spec, damage, crit, scale);
    }

    /// The other classes' status effects one hit of `spec` leaves on enemy `ei`, struck from
    /// `from`.
    pub(super) fn afflict_fx(&mut self, ei: usize, spec: &CastSpec, from: V2) {
        let e = &mut self.enemies[ei];
        let (elite, boss) = (e.elite, e.boss);
        let st = &mut e.status.fx;
        let fx = &spec.fx;
        let elite = elite && !fx.full_control;
        if fx.fear > 0.0 && !boss {
            st.fear = st.fear.max(control_secs(fx.fear, elite, false));
            st.fear_from = from;
        }
        if fx.root > 0.0 {
            st.root = st.root.max(control_secs(fx.root, elite, boss));
        }
        if fx.weaken > 0.0 {
            if st.weaken <= 0.0 {
                st.weaken_cut = 0.0;
            }
            st.weaken = st.weaken.max(fx.weaken);
            st.weaken_cut = st.weaken_cut.max(fx.weaken_cut);
        }
        if fx.mark > 0.0 && (fx.mark >= st.mark || st.mark_secs <= 0.0) {
            st.mark = fx.mark;
            st.mark_secs = st.mark_secs.max(fx.mark_secs);
        }
        if fx.sunder > 0 {
            st.sunder = (st.sunder + fx.sunder).min(SUNDER_MAX);
            st.sunder_secs = SUNDER_SECS;
        }
        if fx.lost > 0.0 && !boss {
            st.lost = st.lost.max(fx.lost);
        }
    }

    /// A Chain spell's jumps after its hit on `first` at `at`: each to the nearest enemy not yet
    /// hit within reach, each keeping its share of the last.
    pub(super) fn chain(&mut self, by: PlayerId, spec: &CastSpec, first: u32, at: V2) {
        let mut hit = vec![first];
        let mut cur = at;
        let mut scale = 1.0;
        let mut plain = spec.clone();
        plain.fx.jumps = 0;
        for _ in 0..spec.fx.jumps {
            let next = self
                .enemies
                .iter()
                .filter(|e| {
                    e.hp > 0.0 && !hit.contains(&e.id) && e.pos.distance(cur) <= CHAIN_REACH
                })
                .min_by(|a, b| a.pos.distance(cur).total_cmp(&b.pos.distance(cur)))
                .map(|e| (e.id, e.pos));
            let Some((id, pos)) = next else {
                break;
            };
            scale *= spec.fx.jump_keep;
            self.strike(by, &plain, id, scale, cur, Crit::Roll);
            hit.push(id);
            cur = pos;
        }
        if hit.len() > 1 {
            self.events.push(Event::Cast {
                caster: by,
                ability: spec.ability,
                visual: self.content.abilities[spec.ability].visual,
                targets: hit[1..].iter().map(|&id| Target::Enemy(id)).collect(),
                dest: None,
                cooldown: 0.0,
            });
        }
    }

    /// A blow of `amount` at hero `h` from enemy `from`, after armor: its buffs' guard, then a
    /// block charge (the attacker struck back), thorn charges and thorns. What is left of it.
    pub(super) fn ward_blow(&mut self, h: usize, amount: f32, from: Option<u32>) -> f32 {
        let amount = amount * (1.0 - self.heroes[h].spell.fx.guard().min(1.0));
        if amount <= 0.0 {
            return 0.0;
        }
        let amount = amount * self.warrior_guard(h) * self.priest_guard(h);
        let player = self.heroes[h].player;
        let pos = self.heroes[h].pos;
        let banks: Vec<(usize, WardKind)> = self.heroes[h]
            .spell
            .fx
            .banks
            .iter()
            .filter(|b| b.charges > 0)
            .map(|b| (b.ability, b.kind))
            .collect();
        if let Some(&(ability, _)) = banks.iter().find(|b| b.1 == WardKind::Block) {
            self.spend_charge(h, ability);
            let spec = self.spell_spec(h, &self.owned_or_new(h, ability));
            if let Some(enemy) = from {
                self.strike(player, &spec, enemy, 1.0, pos, Crit::Roll);
            }
            self.events.push(Event::Cast {
                caster: player,
                ability,
                visual: self.content.abilities[ability].visual,
                targets: from.map(Target::Enemy).into_iter().collect(),
                dest: None,
                cooldown: 0.0,
            });
            self.warrior_blocked(h, &spec, from);
            self.rogue_blocked(h, &spec, from);
            return 0.0;
        }
        if let (Some(enemy), Some(&(ability, _))) =
            (from, banks.iter().find(|b| b.1 == WardKind::Thorns))
        {
            self.spend_charge(h, ability);
            let spec = self.spell_spec(h, &self.owned_or_new(h, ability));
            self.strike(player, &spec, enemy, 1.0, pos, Crit::Roll);
        }
        let thorns: Vec<(usize, f32)> = self.heroes[h]
            .spell
            .fx
            .buffs
            .iter()
            .filter(|b| b.fx.thorns > 0.0)
            .map(|b| (b.ability, b.fx.thorns))
            .collect();
        if let Some(enemy) = from {
            for (ability, dmg) in thorns {
                self.damage_enemy(player, ability, enemy, dmg, false);
            }
        }
        amount
    }

    /// One charge out of hero `h`'s bank for `ability`.
    pub(super) fn spend_charge(&mut self, h: usize, ability: usize) {
        if let Some(b) = self.heroes[h]
            .spell
            .fx
            .banks
            .iter_mut()
            .find(|b| b.ability == ability)
        {
            b.charges = b.charges.saturating_sub(1);
        }
    }

    /// A killing blow at hero `h`: turned aside by a Soulstone (or a capstone), which leaves it
    /// at a share of its health and sends the enemies around it fleeing. Whether it was.
    pub(super) fn cheat_death(&mut self, h: usize) -> bool {
        let fx = &mut self.heroes[h].spell.fx;
        let Some(i) = fx.buffs.iter().position(|b| b.fx.cheat > 0.0) else {
            return self.rogue_cheat(h);
        };
        let buff = fx.buffs.remove(i);
        let hero = &mut self.heroes[h];
        hero.hp = hero.max_hp * buff.fx.cheat;
        let (player, pos) = (hero.player, hero.pos);
        let near: Vec<usize> = (0..self.enemies.len())
            .filter(|&i| self.enemies[i].pos.distance(pos) <= 10.0)
            .collect();
        let mut fear = CastSpec::base(buff.ability, &self.content.abilities[buff.ability]);
        fear.fx.fear = 3.0;
        for ei in near {
            self.afflict_fx(ei, &fear, pos);
        }
        self.events.push(Event::Cast {
            caster: player,
            ability: buff.ability,
            visual: self.content.abilities[buff.ability].visual,
            targets: vec![Target::Hero(player)],
            dest: None,
            cooldown: 0.0,
        });
        true
    }

    /// The defensive button: hero `h` pressed it, leaving `left` for `landed` (the same spot when
    /// the client did not move it). Its cooldown is the world's; a movement one is the client's
    /// to make, else the world moves the hero itself.
    pub(super) fn defend(&mut self, h: usize, left: V2, landed: V2, client_moved: bool) {
        let class = &self.content.classes[self.heroes[h].class];
        let Some(ability) = class.defensive_ix else {
            // A class without one in its content: the mage's movement augments still answer.
            self.dashed(h, left, landed);
            return;
        };
        if self.heroes[h].spell.fx.defense_cd > 0.0 {
            return;
        }
        let spec = self.spell_spec(h, &Owned::new(ability, 0.0));
        let mut landed = landed;
        if !client_moved && spec.fx.leap > 0.0 {
            let hero = &self.heroes[h];
            landed = self.inside(left + hero.facing.normalize_or_zero() * spec.fx.leap);
            self.heroes[h].pos = landed;
        }
        let fx = &mut self.heroes[h].spell.fx;
        fx.defense_cd = spec.cooldown;
        fx.since_defense = 0.0;
        let kind = self.content.abilities[ability].kind;
        let at = if kind == AbilityKind::Ground {
            left
        } else {
            landed
        };
        self.fire(
            h,
            &spec,
            Shot {
                at: Some(at),
                primary: false,
                ..Shot::plain()
            },
        );
        if spec.fx.leap > 0.0 {
            self.dashed(h, left, landed);
        }
    }

    /// Each step's upkeep on the heroes: the defensive button's cooldown, hiding, buffs, heals
    /// over time, capstone bonuses, and heal charges spending themselves on whoever is hurt.
    pub(super) fn tend_heroes_fx(&mut self) {
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let fx = &mut self.heroes[h].spell.fx;
            fx.defense_cd = (fx.defense_cd - DT).max(0.0);
            fx.since_defense += DT;
            fx.hidden = (fx.hidden - DT).max(0.0);
            let haste = fx.haste();
            for b in &mut fx.buffs {
                b.left -= DT;
            }
            fx.buffs.retain(|b| b.left > 0.0);
            for b in &mut fx.boons {
                b.left = (b.left - DT).max(0.0);
            }
            let mut heal = 0.0;
            for (per, left) in &mut fx.hots {
                heal += *per * DT;
                *left -= DT;
            }
            fx.hots.retain(|&(_, left)| left > 0.0);
            let changed = haste != fx.haste();
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + heal).min(hero.max_hp);
            if changed {
                self.refresh(h);
            }
            self.spend_heal_charges(h);
            self.warrior_tend(h);
            self.rogue_tend(h);
            self.priest_tend(h);
        }
    }

    /// Hero `h`'s heal charges: one goes to the most wounded hero within reach who is below its
    /// threshold.
    fn spend_heal_charges(&mut self, h: usize) {
        let banks: Vec<usize> = self.heroes[h]
            .spell
            .fx
            .banks
            .iter()
            .filter(|b| b.kind == WardKind::Heal && b.charges > 0)
            .map(|b| b.ability)
            .collect();
        for ability in banks {
            let spec = self.spell_spec(h, &self.owned_or_new(h, ability));
            let pos = self.heroes[h].pos;
            let hurt = (0..self.heroes.len())
                .filter(|&t| {
                    let x = &self.heroes[t];
                    x.alive
                        && x.pos.distance(pos) <= ALLY_REACH
                        && x.hp < x.max_hp * spec.fx.heal_below
                })
                .min_by(|&a, &b| {
                    let share = |t: usize| self.heroes[t].hp / self.heroes[t].max_hp;
                    share(a).total_cmp(&share(b))
                });
            let Some(t) = hurt else {
                continue;
            };
            self.spend_charge(h, ability);
            let amount = spec.fx.heal * self.heroes[t].max_hp;
            self.heal_hero(t, amount, Some(&spec));
            self.priest_charge_spent(h, ability, t, amount);
            let (player, target) = (self.heroes[h].player, self.heroes[t].player);
            self.events.push(Event::Cast {
                caster: player,
                ability,
                visual: self.content.abilities[ability].visual,
                targets: vec![Target::Hero(target)],
                dest: None,
                cooldown: 0.0,
            });
        }
    }

    /// Each step's upkeep on the enemies' Fear, Root, Weakened, Marks and Sunder.
    pub(super) fn tend_status_fx(&mut self) {
        for e in &mut self.enemies {
            let st = &mut e.status.fx;
            st.fear = (st.fear - DT).max(0.0);
            st.root = (st.root - DT).max(0.0);
            st.weaken = (st.weaken - DT).max(0.0);
            st.lost = (st.lost - DT).max(0.0);
            st.mark_secs = (st.mark_secs - DT).max(0.0);
            if st.mark_secs == 0.0 {
                st.mark = 0.0;
            }
            if st.sunder > 0 {
                st.sunder_secs -= DT;
                if st.sunder_secs <= 0.0 {
                    st.sunder = 0;
                }
            }
        }
    }

    /// The other classes' part of what dying enemies set off.
    pub(super) fn class_deaths(&mut self, deaths: &[Death]) {
        self.warrior_deaths(deaths);
        self.rogue_deaths(deaths);
        self.priest_deaths(deaths);
    }

    /// After a nova of `spec` by hero `h` hit `hit` around `center`: the capstones that count
    /// what a nova reached.
    pub(super) fn after_nova(&mut self, h: usize, spec: &CastSpec, center: V2, hit: &[u32]) {
        if spec.fx.heal > 0.0 {
            // A nova that heals (Holy Nova): every hero in its reach.
            let near: Vec<usize> = (0..self.heroes.len())
                .filter(|&t| {
                    self.heroes[t].alive && self.heroes[t].pos.distance(center) <= spec.radius
                })
                .collect();
            for t in near {
                let amount = spec.fx.heal * self.heroes[t].max_hp;
                let spare = amount - (self.heroes[t].max_hp - self.heroes[t].hp);
                self.heal_hero(t, amount, Some(spec));
                if spare > 0.0 {
                    self.priest_overheal(h, t, spec, spare);
                }
            }
        }
        self.warrior_nova(h, spec, center, hit);
        self.rogue_nova(h, spec, center, hit);
        self.priest_nova(h, spec, center, hit);
    }

    /// Where a ground area of `spec` cast from `origin` is put: at the origin for a totem, a trap
    /// or a spell aimed around its caster (unless Targeted), else `None` (the world's own
    /// choice).
    pub(super) fn ground_spot(&self, h: usize, spec: &CastSpec, origin: V2) -> Option<V2> {
        let own_spot = spec.targeting == Targeting::Around || spec.fx.trap || spec.fx.strikes > 0;
        if !own_spot {
            return None;
        }
        if spec.targeted {
            return self.densest(self.heroes[h].pos, 25.0, spec.radius);
        }
        Some(origin)
    }

    /// One beat of a summon or totem of `spec` at `pos`, or of a healing ground: a summon strikes
    /// the enemies it picks, a healing ground heals the heroes inside (and strikes instead when
    /// nobody inside needs it). Whether it did its own work (else the ground hits everything in it).
    pub(super) fn ground_beat(&mut self, by: PlayerId, spec: &CastSpec, pos: V2) -> bool {
        let Some(h) = self.hero_ix(by) else {
            return true;
        };
        let mut healed_any = false;
        if spec.fx.heal > 0.0 {
            let inside: Vec<usize> = (0..self.heroes.len())
                .filter(|&t| {
                    self.heroes[t].alive && self.heroes[t].pos.distance(pos) <= spec.radius
                })
                .collect();
            let mut healed = 0.0;
            for t in inside {
                let amount = spec.fx.heal * self.heroes[t].max_hp;
                let got = self.heal_hero(t, amount, Some(spec));
                healed += got;
                if got > 0.0 {
                    healed_any = true;
                }
            }
            self.priest_ground_healed(h, spec, healed);
            if spec.fx.strikes == 0 {
                // A healing ground that also burns what stands in it.
                return spec.damage <= 0.0;
            }
        }
        if spec.fx.strikes == 0 {
            return false;
        }
        if healed_any {
            return true;
        }
        let targets = self.pick_targets(
            pos,
            spec.range,
            spec.fx.strikes as usize,
            spec.targeting == Targeting::ElitesFirst,
            None,
        );
        if targets.is_empty() {
            return true;
        }
        self.events.push(Event::Cast {
            caster: by,
            ability: spec.ability,
            visual: self.content.abilities[spec.ability].visual,
            targets: targets.iter().map(|&(id, _)| Target::Enemy(id)).collect(),
            dest: None,
            cooldown: 0.0,
        });
        for (id, _) in targets {
            self.strike(by, spec, id, 1.0, pos, Crit::Roll);
        }
        true
    }

    /// An armed trap at `pos`: whether an enemy has stepped into it.
    pub(super) fn trap_sprung(&self, spec: &CastSpec, pos: V2) -> bool {
        self.enemies
            .iter()
            .any(|e| e.hp > 0.0 && e.pos.distance(pos) <= spec.radius)
    }

    /// Seconds an unsprung trap waits.
    pub(super) fn trap_wait() -> f32 {
        TRAP_WAIT
    }

    /// The index of `player`'s hero.
    pub(super) fn hero_ix(&self, player: PlayerId) -> Option<usize> {
        self.heroes.iter().position(|x| x.player == player)
    }

    /// How much less enemy `e` deals: Weakened.
    pub(super) fn weakness(e: &Enemy) -> f32 {
        if e.status.fx.weaken > 0.0 {
            1.0 - e.status.fx.weaken_cut.max(WEAKEN).min(1.0)
        } else {
            1.0
        }
    }

    /// The `count` enemies within `range` of `from` that do not yet carry hero `h`'s damage over
    /// time from `spec`, nearest first; then those that do.
    pub(super) fn pick_fresh(&self, h: usize, spec: &CastSpec, from: V2) -> Vec<(u32, f32)> {
        let player = self.heroes[h].player;
        let mut near: Vec<(u32, f32, bool)> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0)
            .map(|e| {
                let has = e
                    .status
                    .burns
                    .iter()
                    .any(|b| b.by == player && b.ability == spec.ability);
                (e.id, from.distance(e.pos), has)
            })
            .filter(|&(_, d, _)| d <= spec.range)
            .collect();
        near.sort_by(|a, b| a.2.cmp(&b.2).then(a.1.total_cmp(&b.1)));
        near.truncate(spec.count as usize);
        near.into_iter().map(|(id, d, _)| (id, d)).collect()
    }

    /// A tick of `by`'s damage over time from `ability` on enemy `id`, for `dps`.
    pub(super) fn class_tick(&mut self, by: PlayerId, ability: usize, id: u32, dps: f32) {
        let Some(h) = self.hero_ix(by) else {
            return;
        };
        self.priest_tick(h, ability, id, dps);
    }

    /// A blow of `amount` from enemy `from` reached hero `h`'s health, and it still stands.
    pub(super) fn after_blow(&mut self, h: usize, amount: f32, from: Option<u32>) {
        self.priest_blow(h, amount, from);
    }

    /// Hero `h`'s absorbing shield broke.
    pub(super) fn class_shield_broke(&mut self, h: usize) {
        self.priest_shield_broke(h);
    }

    /// A ground area `g` ran out at `pos`.
    pub(super) fn ground_ended(&mut self, g: &super::combat::Ground, pos: V2) {
        self.priest_ground_ended(g, pos);
    }
}

/// Helpers the class tests share: a hero alone in an emptied arena, spells, foes, steps.
#[cfg(test)]
pub(super) mod testkit {
    pub use super::super::*;
    pub use crate::augments::Augment;
    use crate::content::ContentSources;
    pub use crate::spell::Tag;

    pub fn shipped() -> Arc<Content> {
        Arc::new(
            Content::from_ron(ContentSources {
                classes: include_str!("../../../../content/classes.ron"),
                abilities: include_str!("../../../../content/abilities.ron"),
                enemies: include_str!("../../../../content/enemies.ron"),
                zones: include_str!("../../../../content/zones.ron"),
            })
            .expect("shipped content is valid"),
        )
    }

    /// A hero of `class` alone in an emptied arena: no horde, no shrines, no boss, plenty of
    /// health.
    pub fn hero_of(class: &str) -> World {
        let mut w = World::new(shipped(), "elwynn", 5).unwrap();
        w.add_hero(0, class).unwrap();
        w.enemies.clear();
        w.interactables.clear();
        w.camps.clear();
        w.pickups.clear();
        w.spawn_timer = f32::MAX;
        w.next_boss = f32::MAX;
        w.heroes[0].max_hp = 1e6;
        w.heroes[0].hp = 1e6;
        w
    }

    pub fn ability(w: &World, key: &str) -> usize {
        w.content.ability(key).unwrap()
    }

    /// Give hero 0 exactly these spells, each at `rank` with these augments, ready to cast.
    pub fn spells(w: &mut World, list: &[(&str, u8, &[Augment])]) {
        w.heroes[0].abilities = list
            .iter()
            .map(|&(key, rank, augments)| Owned {
                level: rank,
                augments: augments.to_vec(),
                cd: 0.0,
                ..Owned::new(ability(w, key), 0.0)
            })
            .collect();
    }

    /// A standing enemy with `hp`, `dx` and `dy` yards from hero 0; it does not move or strike
    /// unless asked.
    pub fn foe(w: &mut World, dx: f32, dy: f32, hp: f32) -> u32 {
        let pos = w.heroes[0].pos + V2::new(dx, dy);
        let id = w.id();
        w.enemies.push(Enemy {
            id,
            def: 0,
            pos,
            facing: V2::new(1.0, 0.0),
            moving: false,
            hp,
            max_hp: hp,
            damage: 0.0,
            speed: 0.0,
            radius: 0.5,
            elite: false,
            boss: false,
            attack_cd: 99.0,
            last_hit: None,
            home: None,
            status: Status::default(),
        });
        id
    }

    pub fn run(w: &mut World, secs: f32) -> Vec<Event> {
        let mut seen = Vec::new();
        for _ in 0..(secs / DT).round() as usize {
            w.step();
            seen.extend(w.drain_events());
        }
        seen
    }

    pub fn casts_of(events: &[Event], ability: usize) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, Event::Cast { ability: a, .. } if *a == ability))
            .count()
    }

    /// The first target of the first cast of spell `key` among `events`.
    pub fn first_target(w: &World, events: &[Event], key: &str) -> Option<Target> {
        let want = ability(w, key);
        events.iter().find_map(|e| match e {
            Event::Cast {
                ability, targets, ..
            } if *ability == want => targets.first().copied(),
            _ => None,
        })
    }

    /// Hero 0 presses the defensive button, with no client moving it; one step.
    pub fn press_defense(w: &mut World) {
        w.set_input(
            0,
            PlayerInput {
                dash: true,
                ..Default::default()
            },
        );
        w.step();
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;

    #[test]
    fn a_heal_charge_spends_itself_on_a_hurt_ally_and_overflow_strikes_when_full() {
        let mut w = hero_of("warrior");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(&mut w, &[]);
        let ward = ability(&w, "shield_block");
        // A heal ward made from Shield Block's entry, for the test.
        let mut c = (*w.content).clone();
        c.abilities[ward].ward = Some(WardKind::Heal);
        c.abilities[ward].heal = 0.3;
        w.content = Arc::new(c);
        spells(&mut w, &[("shield_block", 1, &[])]);
        w.step();
        assert_eq!(w.heroes[0].spell.fx.charges(ward), 1);
        let max = w.heroes[1].max_hp;
        w.heroes[1].hp = max * 0.3;
        w.step();
        assert!(w.heroes[1].hp > max * 0.55, "the friend was not healed");
        assert_eq!(w.heroes[0].spell.fx.charges(ward), 0);
    }

    #[test]
    fn a_buff_aura_reaches_allies_and_runs_out() {
        let mut w = hero_of("warrior");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        let mut spec = CastSpec::base(0, &w.content.abilities[0]);
        spec.tags = spec.tags.with(Tag::Aura);
        spec.fx.buff = BuffFx {
            secs: 1.0,
            power: 0.2,
            haste: 0.5,
            ..BuffFx::default()
        };
        let speed = w.heroes[1].speed;
        w.buff(0, &spec);
        assert!(w.heroes[1].spell.fx.power() > 0.19);
        assert!(w.heroes[1].speed > speed * 1.4, "no haste");
        run(&mut w, 1.2);
        assert_eq!(w.heroes[1].spell.fx.power(), 0.0);
        assert!((w.heroes[1].speed - speed).abs() < 1e-3, "the haste stayed");
    }

    #[test]
    fn a_hidden_hero_is_lost_to_the_horde() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        w.enemies[0].speed = 5.0;
        w.heroes[0].spell.fx.hidden = 1.0;
        let at = w.enemy(id).unwrap().pos;
        run(&mut w, 0.5);
        assert!(
            w.enemy(id).unwrap().pos.distance(at) < 0.01,
            "it still chased"
        );
    }

    #[test]
    fn a_rooted_enemy_stays_put_but_strikes_what_reaches_it() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        w.enemies[0].speed = 5.0;
        w.enemies[0].status.fx.root = 2.0;
        let at = w.enemy(id).unwrap().pos;
        run(&mut w, 0.5);
        assert!(
            w.enemy(id).unwrap().pos.distance(at) < 0.01,
            "a rooted enemy moved"
        );
    }

    #[test]
    fn a_soulstone_buff_turns_a_killing_blow_aside() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[]);
        let ab = ability(&w, "charge");
        w.grant(
            0,
            0,
            ab,
            BuffFx {
                secs: 60.0,
                cheat: 0.4,
                ..BuffFx::default()
            },
        );
        w.heroes[0].max_hp = 100.0;
        w.heroes[0].hp = 10.0;
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.hurt_hero_from(0, 50.0, Some(id));
        assert!(w.heroes[0].alive);
        assert!((w.heroes[0].hp - 40.0).abs() < 1e-3);
        assert!(w.enemy(id).unwrap().status.fx.fear > 0.0, "nothing fled");
    }
}

/// The other classes' general augments, on warrior spells.
#[cfg(test)]
mod general_tests {
    use super::testkit::*;

    #[test]
    fn lacerate_stacks_a_bleed_three_times() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("rend", 5, &[Augment::Lacerate])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        for _ in 0..4 {
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
        }
        assert_eq!(w.enemy(id).unwrap().status.burns.len(), 3);
    }

    #[test]
    fn sweeping_and_thunderstrike_hit_one_more_enemy() {
        for a in [Augment::Sweeping, Augment::Thunderstrike] {
            let mut w = hero_of("warrior");
            spells(&mut w, &[("heroic_strike", 5, &[a])]);
            foe(&mut w, 3.0, 0.0, 1e6);
            let beside = foe(&mut w, 4.0, 1.0, 1e6);
            w.step();
            assert!(w.enemy(beside).unwrap().hp < 1e6, "{a:?} missed");
        }
    }

    #[test]
    fn sanguine_turns_spare_lifesteal_into_a_shield() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("bloodthirst", 5, &[Augment::Sanguine])]);
        foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].spell.shield > 0.0, "no shield at full health");
    }

    #[test]
    fn deep_reserves_and_overflowing_grow_a_ward() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("shield_block", 5, &[Augment::DeepReserves])]);
        let block = ability(&w, "shield_block");
        for _ in 0..6 {
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
        }
        assert_eq!(w.heroes[0].spell.fx.charges(block), 5);

        let mut w = hero_of("warrior");
        spells(&mut w, &[("shield_block", 5, &[Augment::Overflowing])]);
        let a = foe(&mut w, 3.0, 0.0, 1e6);
        let b = foe(&mut w, 4.0, 0.0, 1e6);
        for _ in 0..4 {
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
        }
        assert!(w.enemy(a).unwrap().hp < 1e6 && w.enemy(b).unwrap().hp < 1e6);
    }

    #[test]
    fn dread_and_crushing_hit_harder_on_what_is_held() {
        let w = hero_of("warrior");
        let mut s = w.spec_of(0, "intimidating_shout").unwrap();
        crate::augments::Augment::Dread.shape(&mut s);
        assert_eq!(s.fx.vs_feared, 0.25);
        let mut s = w.spec_of(0, "charge").unwrap();
        crate::augments::Augment::Crushing.shape(&mut s);
        assert_eq!(s.fx.vs_stunned, 0.4);
        let mut w = hero_of("warrior");
        spells(&mut w, &[("heroic_strike", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.enemies[0].status.fx.fear = 2.0;
        let mut feared = w.spec_of(0, "heroic_strike").unwrap();
        feared.fx.vs_feared = 0.25;
        let plain = w.spec_of(0, "heroic_strike").unwrap();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        assert!((w.hit_scale(0, ei, &feared) / w.hit_scale(0, ei, &plain) - 1.25).abs() < 1e-4);
    }

    #[test]
    fn booming_voice_and_commanding_grow_a_shout() {
        let w = hero_of("warrior");
        let plain = w.spec_of(0, "demoralizing_shout").unwrap();
        let mut s = plain.clone();
        Augment::BoomingVoice.shape(&mut s);
        Augment::Commanding.shape(&mut s);
        assert!((s.radius - plain.radius * 1.5).abs() < 1e-4);
        assert!((s.fx.weaken - plain.fx.weaken * 1.5).abs() < 1e-4);
    }

    #[test]
    fn crusader_heals_on_a_hit_and_hemorrhage_hurts_the_bleeding() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("heroic_strike", 5, &[Augment::Crusader])]);
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 100.0;
        foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!((w.heroes[0].hp - 120.0).abs() < 1e-3);

        let mut w = hero_of("warrior");
        spells(&mut w, &[("rend", 5, &[Augment::Hemorrhage])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let hs = w.spec_of(0, "heroic_strike").unwrap();
        assert!((w.hit_scale(0, ei, &hs) - 1.2).abs() < 1e-4);
    }

    #[test]
    fn brute_force_takes_fear_too() {
        let w = hero_of("warrior");
        let mut s = w.spec_of(0, "intimidating_shout").unwrap();
        assert!(Augment::BruteForce.fits(&s, &[]));
        Augment::BruteForce.shape(&mut s);
        assert_eq!(s.fx.fear, 0.0);
        assert!(!s.tags.has(Tag::Fear));
    }
}
