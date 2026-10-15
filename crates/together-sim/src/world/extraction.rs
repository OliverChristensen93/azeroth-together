//! Extraction mode's rules ([`crate::extraction`]): bounds and where things may stand, heat,
//! spawning by heat, hunters, letting far enemies go, and the exits and their flights. Each runs
//! only in an extraction run; an arena never reaches them.

use super::*;
use crate::extraction::ExtractionDef;

impl World {
    /// Where heroes may go: the arena's leash, or the extraction map's outline.
    pub fn bounds(&self) -> Bounds {
        match self.map(&self.content) {
            Some(def) => def.bounds(),
            None => Bounds::Circle {
                center: self.center(),
                radius: self.zone().leash,
            },
        }
    }

    /// The extraction map this run plays, read from `content` (the world's own, or a clone of it
    /// held while the world changes); `None` in the arena.
    fn map<'c>(&self, content: &'c Content) -> Option<&'c ExtractionDef> {
        match &self.mode {
            Mode::Extraction(x) => content.extractions.get(x.def),
            Mode::Arena => None,
        }
    }

    fn run_mut(&mut self) -> Option<&mut Extraction> {
        match &mut self.mode {
            Mode::Extraction(x) => Some(x),
            Mode::Arena => None,
        }
    }

    /// Where an interactable put at `pos` stands: there, in the arena; in an extraction run,
    /// inside the outline and out past the safe ring's margin.
    pub(super) fn placeable(&self, pos: V2) -> V2 {
        let Some(def) = self.map(&self.content) else {
            return pos;
        };
        let bounds = def.bounds();
        let at = bounds.clamp(pos);
        if !def.in_margin(at) {
            return at;
        }
        let home = def.safe_center();
        let out = (at - home).normalize_or_zero();
        let out = if out == V2::ZERO { V2::new(1.0, 0.0) } else { out };
        bounds.clamp(home + out * (def.safe.radius + def.safe.margin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::tests::test_content;

    /// An extraction run on the test map with `heroes` mages, settings as `settings` says.
    pub(super) fn run(settings: &str, heroes: u8) -> World {
        let mut w = World::new(Arc::new(test_content(settings)), "goldshire", 7).unwrap();
        for p in 0..heroes {
            w.add_hero(p, "mage").unwrap();
        }
        w
    }

    /// No horde: only what a test puts there.
    pub(super) fn hush(w: &mut World) {
        w.spawn_timer = f32::MAX;
    }

    pub(super) fn at(w: &mut World, player: PlayerId, x: f32, y: f32) {
        w.heroes.iter_mut().find(|h| h.player == player).unwrap().pos = V2::new(x, y);
    }

    pub(super) fn secs(w: &mut World, s: f32) {
        for _ in 0..(s * TICK_HZ as f32).round() as usize {
            w.step();
        }
    }

    pub(super) fn state(w: &World) -> &Extraction {
        match &w.mode {
            Mode::Extraction(x) => x,
            Mode::Arena => panic!("not an extraction run"),
        }
    }

    pub(super) fn state_mut(w: &mut World) -> &mut Extraction {
        match &mut w.mode {
            Mode::Extraction(x) => x,
            Mode::Arena => panic!("not an extraction run"),
        }
    }

    #[test]
    fn an_extraction_zone_plays_extraction_and_starts_with_its_exits_shut() {
        let w = run("", 1);
        let x = state(&w);
        assert_eq!(x.exits.len(), 2);
        assert!(x.exits.iter().all(|e| !e.open));
        assert_eq!(w.bounds(), w.content().extractions[0].bounds());
    }

    #[test]
    fn an_arena_zone_plays_the_arena() {
        let w = World::new(Arc::new(crate::content::tests::sample()), "elwynn", 1).unwrap();
        assert_eq!(w.mode, Mode::Arena);
        assert_eq!(
            w.bounds(),
            Bounds::Circle {
                center: w.center(),
                radius: 150.0
            }
        );
    }

    #[test]
    fn a_pose_past_the_outline_is_held_on_it_and_one_inside_is_taken() {
        let mut w = run("", 1);
        hush(&mut w);
        let pose = |x: f32| PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, 0.0),
                facing: V2::new(1.0, 0.0),
            }),
            ..Default::default()
        };
        w.set_input(0, pose(500.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(300.0, 0.0));
        w.set_input(0, pose(290.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(290.0, 0.0));
    }

    #[test]
    fn nothing_is_placed_in_the_safe_ring_or_past_the_outline() {
        let mut w = run("", 1);
        let near = w.place(InteractableKind::Chest, V2::new(10.0, 0.0));
        let at = w.interactable(near).unwrap().pos;
        assert!((at.distance(V2::ZERO) - 90.0).abs() < 1e-3, "{at:?}");
        let far = w.place(InteractableKind::Chest, V2::new(0.0, 900.0));
        assert_eq!(w.interactable(far).unwrap().pos, V2::new(0.0, 300.0));
    }

    #[test]
    fn a_mirror_gets_the_extraction_state() {
        let mut host = run("", 1);
        state_mut(&mut host).heat = 123.0;
        host.step();
        let mut mirror = run("", 0);
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        assert_eq!(mirror.mode, host.mode);
        assert!(!mirror.hero(0).unwrap().extracted);
    }
}
