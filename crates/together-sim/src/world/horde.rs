//! Where the horde comes from. Around a hero on the move it comes mostly from behind and from the
//! flanks, never straight ahead, so there is always a way forward to run; a hero who has not yet
//! moved is beset from anywhere. Halfway to each boss a swarm ([`Event::Surrounded`]) sends it from
//! every side for a few seconds, and thicker.

use super::*;

/// How quickly a hero's heading follows the way it runs, per step: about a third of a second to
/// turn, so a jink does not swing the horde round.
const STEER: f32 = 0.1;
/// Of the spawns around a hero on the move: this share comes from behind (within 60 degrees of
/// straight back), this share from the flanks (60 to 120), the rest from ahead and aside (120 to
/// [`OPEN_AHEAD`] short of straight ahead).
const BEHIND: f32 = 0.55;
const FLANK: f32 = 0.35;
/// Within this many degrees of the way a hero runs, nothing spawns outside a surround.
const OPEN_AHEAD: f32 = 35.0;
/// A surround's horde, on top of the usual one.
const SURROUND_HORDE: f32 = 0.3;

/// Turn `heading` toward `dir`, a step's movement.
pub(super) fn steer(heading: &mut V2, dir: V2) {
    let dir = dir.normalize_or_zero();
    if dir == V2::ZERO {
        return;
    }
    *heading = if *heading == V2::ZERO {
        dir
    } else {
        (*heading * (1.0 - STEER) + dir * STEER).normalize_or_zero()
    };
}

impl World {
    /// The angle (radians, from +x) at which a spawn comes around a hero heading `heading`.
    pub(super) fn spawn_angle(&mut self, heading: V2) -> f32 {
        if heading == V2::ZERO || self.surround > 0.0 {
            return self.rng.range(0.0, std::f32::consts::TAU);
        }
        let back = (-heading.y).atan2(-heading.x);
        let roll = self.rng.f32();
        // Degrees off straight back.
        let (lo, hi) = if roll < BEHIND {
            (0.0, 60.0)
        } else if roll < BEHIND + FLANK {
            (60.0, 120.0)
        } else {
            (120.0, 180.0 - OPEN_AHEAD)
        };
        let off = self.rng.range(lo, hi).to_radians();
        let side = if self.rng.f32() < 0.5 { -1.0 } else { 1.0 };
        back + off * side
    }

    /// Each step: a swarm runs out, and the next is due halfway to the next boss, once for each
    /// boss to come (none while a boss is up). [`World::next_surround`] says when: the run
    /// panel's one timer counts down to it, then to the boss.
    pub(super) fn tend_surround(&mut self) {
        if self.surround > 0.0 {
            self.surround = (self.surround - DT).max(0.0);
        }
        let boss_up = self.enemies.iter().any(|e| e.boss);
        let done = self.swarm_for == Some(self.next_boss);
        self.next_surround = if boss_up || done {
            f32::INFINITY
        } else {
            self.next_boss - BOSS_EVERY * 0.5
        };
        if self.elapsed < self.next_surround {
            return;
        }
        let secs = self.content.rules.surround_secs;
        self.swarm_for = Some(self.next_boss);
        self.next_surround = f32::INFINITY;
        self.surround = secs;
        self.events.push(Event::Surrounded { secs });
    }

    /// The extra horde a surround brings, on top of [`Difficulty::horde`].
    pub(super) fn surround_horde(&self) -> f32 {
        if self.surround > 0.0 {
            SURROUND_HORDE
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::sample;

    fn world() -> World {
        let mut w = World::new(Arc::new(sample()), "elwynn", 7).unwrap();
        w.add_hero(0, "mage").unwrap();
        w
    }

    /// The angle between two directions, in degrees, 0 to 180.
    fn apart(a: V2, b: V2) -> f32 {
        let d = (a.y.atan2(a.x) - b.y.atan2(b.x)).rem_euclid(std::f32::consts::TAU);
        d.min(std::f32::consts::TAU - d).to_degrees()
    }

    #[test]
    fn the_heading_follows_the_way_a_hero_runs() {
        let mut h = V2::ZERO;
        steer(&mut h, V2::new(3.0, 0.0));
        assert_eq!(h, V2::new(1.0, 0.0));
        for _ in 0..60 {
            steer(&mut h, V2::new(0.0, 1.0));
        }
        assert!(apart(h, V2::new(0.0, 1.0)) < 2.0, "{h:?}");
    }

    #[test]
    fn a_running_hero_has_the_way_ahead_open_and_most_spawns_behind() {
        let mut w = world();
        let ahead = V2::new(1.0, 0.0);
        let mut behind = 0;
        for _ in 0..2000 {
            let dir = V2::from_angle(w.spawn_angle(ahead), 1.0);
            let off = apart(dir, ahead);
            assert!(
                off >= OPEN_AHEAD - 0.01,
                "a spawn {off} degrees off straight ahead"
            );
            if off >= 120.0 {
                behind += 1;
            }
        }
        assert!(behind > 900, "{behind} of 2000 from behind");
    }

    #[test]
    fn a_swarm_comes_halfway_to_the_boss_from_every_side_then_ends() {
        let mut w = world();
        w.tend_surround();
        assert_eq!(w.next_surround, BOSS_EVERY * 0.5);
        w.elapsed = BOSS_EVERY * 0.5;
        w.tend_surround();
        assert!(w.surround > 0.0);
        assert!(w
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::Surrounded { .. })));
        let ahead = V2::new(1.0, 0.0);
        let front = (0..2000)
            .filter(|_| apart(V2::from_angle(w.spawn_angle(ahead), 1.0), ahead) < 30.0)
            .count();
        assert!(front > 100, "only {front} from ahead in a surround");
        for _ in 0..(w.content.rules.surround_secs / DT) as usize + 2 {
            w.tend_surround();
        }
        assert_eq!(w.surround, 0.0);
        // One swarm for this boss: the timer now waits for the boss.
        assert!(w.next_surround.is_infinite());
    }

    #[test]
    fn enemies_run_faster_than_their_entry_but_not_past_the_cap() {
        let mut w = world();
        let rules = w.content.rules.clone();
        let def = 0;
        let entry = w.content.enemies[def].speed;
        let id = w.spawn_enemy(def, w.center() + V2::new(30.0, 0.0), w.center(), false);
        let speed = w.enemy(id).unwrap().speed;
        let want = (entry * rules.enemy_speed).min(rules.enemy_speed_cap.max(entry));
        assert!((speed - want).abs() < 1e-4, "{speed} vs {want}");
        assert!(speed <= rules.enemy_speed_cap.max(entry));
    }
}
