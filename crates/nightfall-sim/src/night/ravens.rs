//! The ravens (spec §7.1): an alarm on the ground. Each of the map's flocks is out in a raid at
//! `ravens.odds`; a settled flock takes off when a hero runs within `run_within` yards or fights
//! within `fight_within`, a 40-yard noise made by that hero, and settles again `back_after`
//! seconds later once no hero is within `clear` yards.

use crate::combat::Dice;
use crate::raid::seed_for;
use crate::{
    Content, Event, Loudness, RaidRoll, Stance, World, FLOCK_FLOWN, FLOCK_GONE, FLOCK_SETTLED, V3,
};

use super::Night;

/// Mixed into the raid's seed (hashed, so seeds one bit apart roll different flocks) for the
/// flocks' dice.
const RAVEN_STREAM: u64 = 0x0BAD_BEA7_C40F_FEE5;

/// One flock in this raid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flock {
    /// [`FLOCK_GONE`], [`FLOCK_SETTLED`] or [`FLOCK_FLOWN`].
    pub(crate) state: u8,
    /// When it last took off or settled.
    pub(crate) since: f32,
}

impl Night {
    /// The night a raid begins with: its flocks rolled from the raid's seed.
    pub(crate) fn new(content: &Content, roll: &RaidRoll) -> Self {
        let mut dice = Dice::new(seed_for(roll.seed, RAVEN_STREAM));
        let odds = content.rules.night.ravens.odds;
        Self {
            flocks: content
                .map
                .flocks
                .iter()
                .map(|_| Flock {
                    state: if dice.chance(odds) {
                        FLOCK_SETTLED
                    } else {
                        FLOCK_GONE
                    },
                    since: 0.0,
                })
                .collect(),
            ..Self::default()
        }
    }
}

impl World {
    /// Settled flocks take off from a running or fighting hero near them, heard; flown ones
    /// settle again in their time.
    pub(crate) fn step_ravens(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.night.ravens;
        let now = self.elapsed;
        for k in 0..self.night.flocks.len().min(content.map.flocks.len()) {
            let at = content.map.flocks[k];
            let f = self.night.flocks[k];
            if f.state == FLOCK_SETTLED {
                let by = self
                    .heroes
                    .iter()
                    .find(|h| {
                        let d = h.pos.distance(at);
                        h.fight.up()
                            && ((h.stance == Stance::Running && d <= r.run_within)
                                || (h.fight.in_combat && d <= r.fight_within))
                    })
                    .map(|h| (h.player, h.z));
                if let Some((player, z)) = by {
                    self.night.flocks[k] = Flock {
                        state: FLOCK_FLOWN,
                        since: now,
                    };
                    let ground = self.map().and_then(|m| m.ground_at(at)).unwrap_or(z);
                    self.make_noise(player, V3::at(at, ground), Loudness::Ravens);
                    self.events.push(Event::RavensFlew { flock: k as u8 });
                }
            } else if f.state == FLOCK_FLOWN
                && now - f.since >= r.back_after
                && !self.heroes.iter().any(|h| h.pos.distance(at) <= r.clear)
            {
                self.night.flocks[k] = Flock {
                    state: FLOCK_SETTLED,
                    since: now,
                };
            }
        }
    }
}
