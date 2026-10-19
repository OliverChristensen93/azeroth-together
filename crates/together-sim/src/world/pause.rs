//! The run held by its players: the game menu pauses it for everyone, and it goes on once every
//! living hero has said Ready ([`PlayerInput::ready`]); and a player giving up
//! ([`PlayerInput::give_up`], or [`World::give_up`] for the whole party).

use super::*;

impl World {
    /// Each step, before anything moves: who has the menu open, who is ready, who gave up.
    pub(super) fn tend_pause(&mut self) {
        let living: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.player)
            .collect();
        let mut quitting = Vec::new();
        for (&player, input) in &mut self.inputs {
            if std::mem::take(&mut input.give_up) {
                quitting.push(player);
            }
            let ready = std::mem::take(&mut input.ready);
            if !living.contains(&player) {
                continue;
            }
            if input.menu {
                // A menu opened: the run waits, and this player is no longer ready.
                self.menu_pause = true;
                self.pause_ready.retain(|&p| p != player);
            } else if ready && self.menu_pause && !self.pause_ready.contains(&player) {
                self.pause_ready.push(player);
            }
        }
        for player in quitting {
            self.give_up(Some(player));
        }
        // Everyone still standing is ready, and nobody has the menu open: on with the run.
        let menus = living
            .iter()
            .any(|p| self.inputs.get(p).is_some_and(|i| i.menu));
        let all_ready = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .all(|h| self.pause_ready.contains(&h.player));
        if self.menu_pause && !menus && all_ready {
            self.menu_pause = false;
            self.pause_ready.clear();
        }
    }

    /// `player` gives up: their hero falls for good, and no friend can raise it. `None`, the
    /// whole party gives up and the run is over.
    pub fn give_up(&mut self, player: Option<PlayerId>) {
        let mut fell = Vec::new();
        for hero in &mut self.heroes {
            if player.is_some_and(|p| p != hero.player) {
                continue;
            }
            if !self.quit.contains(&hero.player) {
                self.quit.push(hero.player);
            }
            if hero.alive {
                hero.alive = false;
                hero.hp = 0.0;
                hero.moving = false;
                hero.revive = 0.0;
                fell.push(hero.player);
            }
        }
        for player in fell {
            self.events.push(Event::HeroDied { player });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::sample;

    fn party() -> World {
        let mut w = World::new(Arc::new(sample()), "elwynn", 2).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.add_hero(1, "mage").unwrap();
        w.enemies.clear();
        w
    }

    fn input(w: &mut World, player: PlayerId, f: impl FnOnce(&mut PlayerInput)) {
        let mut i = PlayerInput::default();
        f(&mut i);
        w.queue_input(player, i);
    }

    #[test]
    fn a_menu_pauses_everyone_until_everyone_is_ready() {
        let mut w = party();
        input(&mut w, 0, |i| i.menu = true);
        w.step();
        assert!(w.held());
        let at = w.elapsed;
        // The menu closes: still waiting, for both.
        input(&mut w, 0, |_| {});
        w.step();
        assert!(w.held());
        input(&mut w, 0, |i| i.ready = true);
        w.step();
        assert!(w.held(), "player 1 is not ready yet");
        assert_eq!(w.elapsed, at);
        input(&mut w, 1, |i| i.ready = true);
        w.step();
        assert!(!w.held());
    }

    #[test]
    fn giving_up_fells_the_hero_for_good() {
        let mut w = party();
        input(&mut w, 1, |i| i.give_up = true);
        w.step();
        let h = w.hero(1).unwrap();
        assert!(!h.alive);
        // Player 0 stands beside it long enough to raise anyone else.
        w.heroes[0].pos = w.heroes[1].pos;
        for _ in 0..(REVIVE_SECS / DT) as usize + 10 {
            w.step();
        }
        assert!(!w.hero(1).unwrap().alive);
        assert!(!w.game_over());
        w.give_up(None);
        assert!(w.game_over());
    }
}
