//! Heroes falling (spec §8.1): at 0 health a hero is downed for `downed_for` seconds and crawls; a
//! teammate up and standing by them for `revive_for` seconds gets them up (a 20-yard noise as the
//! revive begins), with `revive_health` of their health; after that they are dead, the body where
//! it fell (sub-project 5 leaves their gear on it). Enemies leave the downed and the dead be:
//! the senses neither see nor hear them, and an enemy after them is made to lose them at once, so
//! it searches where they fell, which gives a friend a chance.

use super::{Event, Life, Stop, UnitRef};
use crate::{Loudness, PlayerId, World, DT, V3};

impl World {
    /// `player`'s hero takes `amount` damage from nowhere in particular (a fall, a trap, a test
    /// hook); an enemy's blow goes through [`World::wound`] with its maker. An amount that is not
    /// a positive number does nothing.
    pub fn hurt_hero(&mut self, player: PlayerId, amount: f32) {
        self.wound(player, amount, None);
    }

    /// `player`'s hero takes `amount` damage `by` something: it breaks their stealth (spec §6.3),
    /// ends their meal, keeps them in combat, and at 0 health downs them. Nothing happens to a
    /// hero who is not up, nor for an amount that is not a positive number (health never becomes
    /// one that is not a number).
    pub(crate) fn wound(&mut self, player: PlayerId, amount: f32, by: Option<UnitRef>) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if !self.heroes[hi].fight.up() || !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let linger = self.content.rules.combat.combat_linger;
        let h = &mut self.heroes[hi];
        h.hp = (h.hp - amount).max(0.0);
        let down = h.hp <= 0.0;
        self.events.push(Event::Damage {
            to: UnitRef::Hero(player),
            by,
            amount,
            crit: false,
            ambush: false,
        });
        self.break_stealth(player);
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].combat_for = linger;
        self.stop_eating(hi);
        if down {
            self.down(hi);
        }
    }

    /// Hero `hi` goes down: their cast stops, they leave stealth and stop swinging, and every
    /// enemy after them loses them.
    fn down(&mut self, hi: usize) {
        let content = self.content.clone();
        self.stop_cast(hi, Stop::Interrupted);
        let h = &mut self.heroes[hi];
        let player = h.player;
        h.hp = 0.0;
        h.stealthed = false;
        h.fight.life = Life::Downed {
            left: content.rules.combat.downed_for,
            revive: 0.0,
        };
        h.fight.attacking = None;
        h.fight.sprint = 0.0;
        self.senses.lose(player, &content.rules.senses.awareness);
        // Their stance and pace now: the crawl shows in the next snapshot, not a step late.
        self.rederive(player);
        self.events.push(Event::Downed { player });
    }

    /// A downed hero's clock: a teammate up within `revive_reach` revives them, a 20-yard noise
    /// as the revive begins, and walking away (or going down) starts it over; at `downed_for`
    /// they die.
    pub(crate) fn step_fall(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let Life::Downed { left, revive } = self.heroes[hi].fight.life else {
            return;
        };
        let (player, pos, z) = (
            self.heroes[hi].player,
            self.heroes[hi].pos,
            self.heroes[hi].z,
        );
        let reviver = self
            .heroes
            .iter()
            .find(|o| o.player != player && o.fight.up() && o.pos.distance(pos) <= r.revive_reach)
            .map(|o| o.player);
        let left = left - DT;
        let revive = match reviver {
            Some(by) => {
                if revive <= 0.0 {
                    self.make_noise(by, V3::at(pos, z), Loudness::Normal);
                }
                revive + DT
            }
            None => 0.0,
        };
        match reviver {
            Some(by) if revive + 1e-4 >= r.revive_for => {
                let h = &mut self.heroes[hi];
                h.fight.life = Life::Up;
                h.hp = h.max_hp * r.revive_health;
                self.events.push(Event::Revived { player, by });
            }
            _ if left <= 1e-4 => {
                self.heroes[hi].fight.life = Life::Dead;
                self.events.push(Event::Died { player });
            }
            _ => self.heroes[hi].fight.life = Life::Downed { left, revive },
        }
    }
}
