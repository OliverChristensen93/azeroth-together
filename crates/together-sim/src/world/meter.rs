//! The damage meter's other numbers: what went into bosses (kept beside the damage itself, in
//! [`World::damage_enemy`]), what shields soaked up (in [`World::hurt_hero_from`]), and healing,
//! by the spell that did it.
//!
//! Healing is read off the heroes' health rather than counted at the thirty places that heal.
//! Each spell cast, triggered cast and landing missile runs inside a healing context
//! ([`World::heal_begin`], [`World::heal_end`]): whatever health any hero gained inside it (blows
//! taken meanwhile put back) is that caster's spell's healing, a heal on a friend included. A
//! heal over time remembers whose spell laid it. What a hero gained over the step beyond all
//! that (regeneration, a health shard, a level-up) is its own, under [`OTHER_HEALING`].

use super::*;

/// The meter's row for healing that came from no spell.
pub const OTHER_HEALING: usize = usize::MAX;

/// The heroes' health when a healing context opened: player, health, blows taken, alive.
pub(super) struct HealMark(Vec<(PlayerId, f32, f32, bool)>);

/// Add `amount` to `ability`'s running total in `by`.
pub(super) fn add_to(by: &mut Vec<(usize, f64)>, ability: usize, amount: f32) {
    match by.iter_mut().find(|(a, _)| *a == ability) {
        Some((_, total)) => *total += f64::from(amount),
        None => by.push((ability, f64::from(amount))),
    }
}

impl World {
    /// The start of a step: each living hero's health, to tell its healing by at the end.
    pub(super) fn mark_healing(&mut self) {
        for hero in &mut self.heroes {
            hero.hp_mark = hero.alive.then_some(hero.hp);
            hero.taken = 0.0;
        }
        self.heal_credit.clear();
    }

    /// Open a healing context for `by`'s spell `ability`; `None` inside one already, which
    /// keeps what happens within (a triggered cast) the outer spell's.
    pub(super) fn heal_begin(&mut self, by: PlayerId, ability: usize) -> Option<HealMark> {
        if self.heal_source.is_some() {
            return None;
        }
        self.heal_source = Some((by, ability));
        Some(HealMark(
            self.heroes
                .iter()
                .map(|h| (h.player, h.hp, h.taken, h.alive))
                .collect(),
        ))
    }

    /// Close a healing context: what every hero gained since `mark` is the source spell's.
    pub(super) fn heal_end(&mut self, mark: Option<HealMark>) {
        let Some(mark) = mark else {
            return;
        };
        let Some((by, ability)) = self.heal_source.take() else {
            return;
        };
        for (player, hp, taken, alive) in mark.0 {
            let Some(hero) = self.heroes.iter().find(|h| h.player == player) else {
                continue;
            };
            if !alive || !hero.alive {
                continue;
            }
            let gained = hero.hp - hp + (hero.taken - taken);
            if gained > 1e-4 {
                self.heal_credit.push((player, by, ability, gained));
            }
        }
    }

    /// Who a heal laid now (a heal over time) belongs to: the open context's spell, else the
    /// hero's own other healing.
    pub(super) fn heal_owner(&self, target: PlayerId) -> (PlayerId, usize) {
        self.heal_source.unwrap_or((target, OTHER_HEALING))
    }

    /// A heal of `amount` on `target` from `by`'s spell `ability`, outside any context (a heal
    /// over time ticking).
    pub(super) fn credit_heal(
        &mut self,
        target: PlayerId,
        by: PlayerId,
        ability: usize,
        amount: f32,
    ) {
        if amount > 0.0 {
            self.heal_credit.push((target, by, ability, amount));
        }
    }

    /// The end of a step: what each hero healed goes on the meter, spell by spell.
    pub(super) fn tally_healing(&mut self) {
        let mut got: Vec<(PlayerId, f32)> = Vec::new();
        for hero in &mut self.heroes {
            // A hero raised or fallen this step did not heal by it.
            if let (Some(mark), true) = (hero.hp_mark.take(), hero.alive) {
                let healed = hero.hp - mark + hero.taken;
                if healed > 1e-4 {
                    got.push((hero.player, healed));
                }
            }
        }
        let credits = std::mem::take(&mut self.heal_credit);
        for &(target, by, ability, amount) in &credits {
            let Some(entry) = got.iter_mut().find(|(p, _)| *p == target) else {
                continue;
            };
            let moved = amount.min(entry.1);
            entry.1 -= moved;
            if let Some(healer) = self.heroes.iter_mut().find(|h| h.player == by) {
                healer.healed += f64::from(moved);
                add_to(&mut healer.healed_by, ability, moved);
            }
        }
        for (player, amount) in got {
            if amount <= 1e-4 {
                continue;
            }
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.healed += f64::from(amount);
                add_to(&mut hero.healed_by, OTHER_HEALING, amount);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::sample;

    fn world() -> World {
        let mut w = World::new(Arc::new(sample()), "elwynn", 3).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.enemies.clear();
        w.interactables.clear();
        w
    }

    fn by(h: &Hero, ability: usize) -> f64 {
        h.healed_by
            .iter()
            .find(|(a, _)| *a == ability)
            .map_or(0.0, |x| x.1)
    }

    #[test]
    fn regeneration_counts_as_other_healing_and_a_blow_does_not_hide_it() {
        let mut w = world();
        w.heroes[0].hp = 50.0;
        w.mark_healing();
        w.heroes[0].hp += 10.0;
        w.hurt_hero(0, 4.0);
        w.tally_healing();
        assert!((w.heroes[0].healed - 10.0).abs() < 1e-3, "{}", w.heroes[0].healed);
        assert!((by(&w.heroes[0], OTHER_HEALING) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn a_heal_inside_a_spell_is_that_spells_and_the_casters() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].hp = 40.0;
        w.mark_healing();
        let mark = w.heal_begin(0, 1);
        w.heal_hero(1, 25.0, None);
        w.heal_end(mark);
        w.tally_healing();
        assert!((w.heroes[0].healed - 25.0).abs() < 1e-3);
        assert!((by(&w.heroes[0], 1) - 25.0).abs() < 1e-3);
        assert_eq!(w.heroes[1].healed, 0.0);
    }

    #[test]
    fn a_heal_over_time_stays_its_casters() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].hp = 40.0;
        // Laid inside player 0's spell 1, ticking later outside any context.
        let mark = w.heal_begin(0, 1);
        let owner = w.heal_owner(1);
        w.heal_end(mark);
        w.heroes[1].spell.fx.hots.push((30.0, 2.0, owner.0, owner.1));
        w.mark_healing();
        w.tend_heroes_fx();
        w.tally_healing();
        assert!(by(&w.heroes[0], 1) > 0.5, "{:?}", w.heroes[0].healed_by);
        assert_eq!(w.heroes[1].healed, 0.0);
    }

    #[test]
    fn a_shield_soaking_a_blow_is_absorbed() {
        let mut w = world();
        w.heroes[0].spell.shield = 30.0;
        w.hurt_hero(0, 20.0);
        assert!(w.heroes[0].absorbed > 0.0);
    }
}
