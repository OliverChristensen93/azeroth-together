//! The night clock (spec §7.2): each stage's cue `warn` seconds before it lands, so the players
//! feel the clock with no timer on screen; then the stage; and the hunt's exit closing as the
//! roll said. A clock started late (a test hook) gives every passed stage's cue and landing once,
//! cues first, in order.

use crate::{Event, StageDef, World};

/// The stage the clock is in `t` seconds into the raid: the last that has landed.
pub(crate) fn stage_at(stages: &[StageDef], t: f32) -> usize {
    stages.iter().rposition(|s| s.at <= t).unwrap_or(0)
}

impl World {
    /// The clock's step: cues due, stages landed, the hunt's exit closed.
    pub(crate) fn step_clock(&mut self) {
        let content = self.content().clone();
        let stages = &content.rules.night.stages;
        let t = self.elapsed;
        while self.night.cued < stages.len()
            && t >= stages[self.night.cued].at - stages[self.night.cued].warn
        {
            let s = self.night.cued;
            // The first stage is where the raid starts: nothing to announce.
            if s > 0 {
                self.events.push(Event::NightCue { stage: s as u8 });
            }
            self.night.cued += 1;
        }
        let now = stage_at(stages, t);
        while self.night.stage < now {
            self.night.stage += 1;
            self.events.push(Event::NightFell {
                stage: self.night.stage as u8,
            });
            self.stage_landed(self.night.stage);
        }
        if let Some((exit, at)) = self.raid.roll.closes {
            if !self.night.closed && t >= at {
                self.night.closed = true;
                self.events.push(Event::ExitClosed { exit });
            }
        }
    }
}
