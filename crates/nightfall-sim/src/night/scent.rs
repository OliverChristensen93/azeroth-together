//! Scent (spec §7.1, §7.3): every hero who lives leaves a trail, one who crawls after a fall too, a crumb every `scent.every`
//! yards walked, each fading after `scent.lasts` seconds; none in water or on a road, where the
//! trail breaks into a new stretch. A tracker that crosses a fresh crumb grows suspicious of its
//! maker; one suspicious of a hero follows their trail crumb by crumb toward newer ones. At a
//! break, or where the crumb it needs has faded, it has lost them, and its suspicion drains as
//! any enemy's does. A long detour does the same, since a tracker trots slower than a hero runs.

use std::collections::VecDeque;

use crate::combat::{Hold, Life};
use crate::senses::State;
use crate::{PlayerId, World, V2, V3};

/// One crumb of scent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crumb {
    pub(crate) at: V3,
    /// When it was left.
    pub(crate) t: f32,
    /// Which unbroken stretch of the trail it belongs to.
    pub(crate) stretch: u32,
}

/// One hero's trail, oldest crumb first.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trail {
    pub(crate) player: PlayerId,
    pub(crate) crumbs: VecDeque<Crumb>,
    stretch: u32,
    /// The hero stands where no scent is left (water, a road): the trail's end is a break.
    broken: bool,
}

impl Trail {
    /// Where a tracker at `at` goes next on this trail: the crumb after the newest one within
    /// `pick` yards of it, while both are of one stretch; that crumb itself when it is the
    /// trail's end and the trail is whole. `None` with no crumb near, or a break there.
    pub(crate) fn next_from(&self, at: V2, pick: f32) -> Option<V3> {
        let k = self
            .crumbs
            .iter()
            .rposition(|c| c.at.ground().distance(at) <= pick)?;
        match self.crumbs.get(k + 1) {
            Some(n) if n.stretch == self.crumbs[k].stretch => Some(n.at),
            Some(_) => None,
            None if !self.broken => Some(self.crumbs[k].at),
            None => None,
        }
    }
}

impl World {
    /// Every hero who lives leaves their scent, a crawling one too; old crumbs fade.
    pub(crate) fn lay_scent(&mut self) {
        let content = self.content().clone();
        let s = content.rules.night.scent;
        let now = self.elapsed;
        let map = self.map().cloned();
        for h in &self.heroes {
            let i = match self.night.trails.iter().position(|t| t.player == h.player) {
                Some(i) => i,
                None => {
                    self.night.trails.push(Trail {
                        player: h.player,
                        ..Trail::default()
                    });
                    self.night.trails.len() - 1
                }
            };
            let t = &mut self.night.trails[i];
            // A hero crawling after a fall still leaves a trail (a tracker may follow it to them); a
            // body leaves none.
            if matches!(h.fight.life, Life::Dead) {
                continue;
            }
            let wet = map.as_ref().is_some_and(|m| m.is_water(h.pos));
            if wet || content.map.on_road(h.pos) {
                t.broken = true;
                continue;
            }
            let crumb = Crumb {
                at: V3::at(h.pos, h.z),
                t: now,
                stretch: t.stretch,
            };
            if t.broken {
                t.broken = false;
                t.stretch += 1;
                t.crumbs.push_back(Crumb {
                    stretch: t.stretch,
                    ..crumb
                });
            } else if t
                .crumbs
                .back()
                .is_none_or(|c| c.at.ground().distance(h.pos) >= s.every)
            {
                t.crumbs.push_back(crumb);
            }
        }
        for t in &mut self.night.trails {
            while t.crumbs.front().is_some_and(|c| now - c.t > s.lasts) {
                t.crumbs.pop_front();
            }
        }
    }

    /// Trackers in the fight pick up a fresh trail they cross, and follow the one of the hero
    /// they suspect.
    pub(crate) fn follow_scent(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.senses.awareness;
        let pick = content.rules.night.scent.pick;
        for i in 0..self.enemies.len().min(self.senses.minds.len()) {
            let (kind, hp, at) = {
                let e = &self.enemies[i];
                (e.kind, e.hp, e.pos)
            };
            if !content.enemies[kind].tracker || hp <= 0.0 {
                continue;
            }
            if self
                .combat
                .enemies
                .get(i)
                .is_some_and(|c| c.engage.hold == Hold::Out)
            {
                continue;
            }
            let mind = &mut self.senses.minds[i];
            match mind.state {
                State::Calm => {
                    // The newest crumb within reach, of any hero.
                    let found = self
                        .night
                        .trails
                        .iter()
                        .filter_map(|t| {
                            t.crumbs
                                .iter()
                                .filter(|c| c.at.ground().distance(at) <= pick)
                                .max_by(|a, b| a.t.total_cmp(&b.t))
                                .map(|c| (t.player, *c))
                        })
                        .max_by(|a, b| a.1.t.total_cmp(&b.1.t));
                    if let Some((player, c)) = found {
                        mind.sniff(player, c.at, r);
                    }
                }
                State::Wary { player, .. } | State::Checking { player, .. } => {
                    let next = self
                        .night
                        .trails
                        .iter()
                        .find(|t| t.player == player)
                        .and_then(|t| t.next_from(at, pick));
                    if let Some(next) = next {
                        mind.sniff(player, next, r);
                    }
                }
                _ => {}
            }
        }
    }
}
