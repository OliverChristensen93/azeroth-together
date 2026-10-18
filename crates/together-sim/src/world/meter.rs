//! The damage meter's other numbers: what went into bosses (kept beside the damage itself, in
//! [`World::damage_enemy`]), what shields soaked up (in [`World::hurt_hero_from`]), and healing.
//!
//! Healing is read off each hero's health rather than counted at the thirty places that heal:
//! what a step added, with what blows took in it put back, is what it healed. Each hero is
//! credited with its own; a heal one hero's spell lands on a friend while it casts is the
//! caster's instead ([`World::credit_heal`]).

use super::*;

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

    /// A heal of `amount` landed on `target`: while another hero's spells are being cast, it is
    /// that hero's on the meter.
    pub(super) fn credit_heal(&mut self, target: PlayerId, amount: f32) {
        if let Some(by) = self.casting.filter(|&by| by != target) {
            if amount > 0.0 {
                self.heal_credit.push((target, by, amount));
            }
        }
    }

    /// The end of a step: what each hero healed goes on the meter.
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
        for &(target, by, amount) in &self.heal_credit {
            let Some(entry) = got.iter_mut().find(|(p, _)| *p == target) else {
                continue;
            };
            let moved = amount.min(entry.1);
            entry.1 -= moved;
            if let Some(healer) = self.heroes.iter_mut().find(|h| h.player == by) {
                healer.healed += f64::from(moved);
            }
        }
        for (player, amount) in got {
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.healed += f64::from(amount);
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

    #[test]
    fn regeneration_counts_as_healing_and_a_blow_does_not_hide_it() {
        let mut w = world();
        w.heroes[0].hp = 50.0;
        w.mark_healing();
        w.heroes[0].hp += 10.0;
        w.hurt_hero(0, 4.0);
        w.tally_healing();
        assert!(
            (w.heroes[0].healed - 10.0).abs() < 1e-3,
            "{}",
            w.heroes[0].healed
        );
    }

    #[test]
    fn a_heal_cast_on_a_friend_is_the_casters() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].hp = 40.0;
        w.mark_healing();
        w.casting = Some(0);
        w.heal_hero(1, 25.0, None);
        w.casting = None;
        w.tally_healing();
        assert!((w.heroes[0].healed - 25.0).abs() < 1e-3);
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
