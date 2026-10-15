//! Extraction mode's rules ([`crate::extraction`]): bounds and where things may stand, heat,
//! spawning by heat, hunters, letting far enemies go, and the exits and their flights. Each runs
//! only in an extraction run; an arena never reaches them.

use super::*;
use crate::extraction::{ExtractionDef, ExtractionSettings};

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
        let out = if out == V2::ZERO {
            V2::new(1.0, 0.0)
        } else {
            out
        };
        bounds.clamp(home + out * (def.safe.radius + def.safe.margin))
    }
    /// The rules this extraction run tries, or `None` in the arena.
    pub fn extraction_settings(&self) -> Option<ExtractionSettings> {
        self.map(&self.content).map(|d| d.settings)
    }

    /// The heat `player` plays under: the party's when shared, else their own; 0 in the arena.
    pub fn heat_of(&self, player: PlayerId) -> f32 {
        let (Mode::Extraction(x), Some(def)) = (&self.mode, self.map(&self.content)) else {
            return 0.0;
        };
        if def.settings.heat_shared {
            return x.heat;
        }
        x.heat_by
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(x.floor, |&(_, h)| h)
    }

    /// The skull `player` is at: 0 cool, up to the map's hottest.
    pub fn skulls_of(&self, player: PlayerId) -> usize {
        self.map(&self.content)
            .map_or(0, |d| d.skull_index(self.heat_of(player)))
    }

    /// The party's heat: shared, or the hottest hero's.
    fn party_heat(&self) -> f32 {
        match &self.mode {
            Mode::Extraction(x) => x.heat_by.iter().map(|&(_, h)| h).fold(x.heat, f32::max),
            Mode::Arena => 0.0,
        }
    }

    /// XP and gold from a kill by `killer`, times: by the killer's heat, or the party's for a
    /// kill nobody made. 1 in the arena.
    pub(super) fn reward_of(&self, killer: Option<PlayerId>) -> f32 {
        let Some(def) = self.map(&self.content) else {
            return 1.0;
        };
        let heat = killer.map_or_else(|| self.party_heat(), |p| self.heat_of(p));
        def.skull(heat).reward
    }

    /// Luck heat lends `player`'s hands of cards. 0 in the arena.
    pub(super) fn reward_luck(&self, player: PlayerId) -> f32 {
        let Some(def) = self.map(&self.content) else {
            return 0.0;
        };
        (def.skull(self.heat_of(player)).reward - 1.0) * def.heat.reward_luck
    }

    /// `amount` heat: to the party when shared; else to `player`, or with `None` to every
    /// living hero.
    fn add_heat(&mut self, player: Option<PlayerId>, amount: f32) {
        let Some(settings) = self.extraction_settings() else {
            return;
        };
        let living: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.player)
            .collect();
        let Some(x) = self.run_mut() else {
            return;
        };
        if settings.heat_shared {
            x.heat += amount;
            return;
        }
        for p in living
            .into_iter()
            .filter(|p| player.is_none_or(|q| q == *p))
        {
            hero_heat(x, p).1 += amount;
        }
    }

    /// A kill's heat, for `killer` (or, shared, the party).
    pub(super) fn heat_from_kill(&mut self, killer: Option<PlayerId>) {
        let content = self.content.clone();
        if let Some(def) = self.map(&content) {
            self.add_heat(killer, def.heat.kill);
        }
    }

    /// Each step's extraction upkeep. Nothing in the arena.
    pub(super) fn tend_extraction(&mut self) {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        self.tend_heat(def);
        self.lose_them(def);
        self.tell_skulls(def);
    }

    /// The floor rises with the night; heat climbs while anyone (shared) or the hero (not
    /// shared) is outside the safe ring, and cools while everyone (shared) or the hero is home.
    fn tend_heat(&mut self, def: &ExtractionDef) {
        let floor = def.floor(self.elapsed);
        let living: Vec<(PlayerId, bool)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, def.in_safe(h.pos)))
            .collect();
        let Some(x) = self.run_mut() else {
            return;
        };
        x.floor = floor;
        let (outside, cool) = (def.heat.outside * DT, def.safe.cool * DT);
        if def.settings.heat_shared {
            if living.iter().any(|&(_, home)| !home) {
                x.heat += outside;
            } else if !living.is_empty() {
                x.heat -= cool;
            }
            x.heat = x.heat.max(floor);
        } else {
            for &(p, home) in &living {
                let h = &mut hero_heat(x, p).1;
                let next = if home { *h - cool } else { *h + outside };
                *h = next.max(floor);
            }
            for (_, h) in &mut x.heat_by {
                *h = h.max(floor);
            }
        }
        x.peak = x
            .heat_by
            .iter()
            .map(|&(_, h)| h)
            .fold(x.heat.max(x.peak), f32::max);
    }

    /// `lose_them`: once no enemy has been near any living hero for the map's quiet spell, every
    /// heat drops to the start of the skull below it (never under the floor).
    fn lose_them(&mut self, def: &ExtractionDef) {
        if !def.settings.lose_them {
            return;
        }
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        let range = def.heat.quiet_range;
        let near = self
            .enemies
            .iter()
            .any(|e| living.iter().any(|p| p.distance(e.pos) <= range));
        let Some(x) = self.run_mut() else {
            return;
        };
        x.quiet = if near { 0.0 } else { x.quiet + DT };
        if x.quiet < def.heat.quiet_secs {
            return;
        }
        x.quiet = 0.0;
        let floor = x.floor;
        let drop = |h: f32| match def.skull_index(h) {
            0 => h,
            s => def.heat.skulls[s - 1].at.max(floor),
        };
        x.heat = drop(x.heat);
        for (_, h) in &mut x.heat_by {
            *h = drop(*h);
        }
    }

    /// A [`Event::HeatChanged`] for each heat whose skull changed since last told.
    fn tell_skulls(&mut self, def: &ExtractionDef) {
        let now: Vec<(Option<PlayerId>, u8)> = match &self.mode {
            Mode::Extraction(x) if def.settings.heat_shared => {
                vec![(None, def.skull_index(x.heat) as u8)]
            }
            Mode::Extraction(x) => x
                .heat_by
                .iter()
                .map(|&(p, h)| (Some(p), def.skull_index(h) as u8))
                .collect(),
            Mode::Arena => return,
        };
        let Some(x) = self.run_mut() else {
            return;
        };
        let changed: Vec<Event> = now
            .iter()
            .filter(|&&(who, skulls)| {
                let before = x
                    .told
                    .iter()
                    .find(|(w, _)| *w == who)
                    .map_or(0, |&(_, s)| s);
                skulls != before
            })
            .map(|&(player, skulls)| Event::HeatChanged { player, skulls })
            .collect();
        x.told = now;
        self.events.extend(changed);
    }
}

/// `player`'s entry in a per-hero heat list, made at the floor if they have none yet.
fn hero_heat(x: &mut Extraction, player: PlayerId) -> &mut (PlayerId, f32) {
    if !x.heat_by.iter().any(|(p, _)| *p == player) {
        x.heat_by.push((player, x.floor));
    }
    x.heat_by
        .iter_mut()
        .find(|(p, _)| *p == player)
        .expect("just made")
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
        w.heroes
            .iter_mut()
            .find(|h| h.player == player)
            .unwrap()
            .pos = V2::new(x, y);
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

    fn heat_told(w: &mut World) -> Vec<Event> {
        w.drain_events()
            .into_iter()
            .filter(|e| matches!(e, Event::HeatChanged { .. }))
            .collect()
    }

    #[test]
    fn heat_rises_outside_and_cools_only_while_everyone_is_home() {
        let mut w = run("", 2);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 2.0);
        assert!((w.heat_of(0) - 2.0).abs() < 0.1, "{}", w.heat_of(0));
        assert_eq!(w.heat_of(0), w.heat_of(1), "one heat for the party");
        at(&mut w, 0, 10.0, 0.0);
        state_mut(&mut w).heat = 50.0;
        secs(&mut w, 1.0);
        assert!((w.heat_of(0) - 35.0).abs() < 0.1, "{}", w.heat_of(0));
        at(&mut w, 1, 100.0, 0.0);
        secs(&mut w, 1.0);
        assert!((w.heat_of(0) - 36.0).abs() < 0.1, "one out: no cooling");
    }

    #[test]
    fn each_hero_has_their_own_heat_when_it_is_not_shared() {
        let mut w = run("heat_shared: false", 2);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 2.0);
        assert!((w.heat_of(0) - 2.0).abs() < 0.1, "{}", w.heat_of(0));
        assert_eq!(w.heat_of(1), 0.0);
    }

    #[test]
    fn the_night_floor_rises_and_heat_never_cools_below_it() {
        let mut w = run("", 1);
        hush(&mut w);
        w.elapsed = 300.0;
        secs(&mut w, 1.0);
        let floor = state(&w).floor;
        assert!((floor - 80.67).abs() < 0.1, "{floor}");
        assert!((w.heat_of(0) - floor).abs() < 1e-3);
    }

    #[test]
    fn reaching_a_skull_is_told_once() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        state_mut(&mut w).heat = 99.5;
        secs(&mut w, 1.0);
        assert_eq!(
            heat_told(&mut w),
            vec![Event::HeatChanged {
                player: None,
                skulls: 1
            }]
        );
        secs(&mut w, 1.0);
        assert!(heat_told(&mut w).is_empty());
    }

    #[test]
    fn with_lose_them_a_quiet_spell_drops_a_skull() {
        let mut w = run("lose_them: true", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        state_mut(&mut w).heat = 300.0;
        secs(&mut w, 10.5);
        assert_eq!(w.skulls_of(0), 1, "{}", w.heat_of(0));
    }

    #[test]
    fn without_lose_them_heat_holds_through_a_quiet_spell() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        state_mut(&mut w).heat = 300.0;
        secs(&mut w, 10.5);
        assert_eq!(w.skulls_of(0), 2);
    }

    #[test]
    fn hotter_kills_are_worth_more_and_heat_the_party() {
        for (heat, xp) in [(0.0, 1), (1000.0, 5)] {
            let mut w = run("", 1);
            hush(&mut w);
            state_mut(&mut w).heat = heat;
            let id = w.spawn_enemy(0, V2::new(5.0, 0.0), V2::ZERO, false);
            w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
            w.step();
            let dropped = w.drain_events().into_iter().find_map(|e| match e {
                Event::GemDropped { xp, .. } => Some(xp),
                _ => None,
            });
            assert_eq!(dropped, Some(xp), "at {heat} heat");
        }
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        let id = w.spawn_enemy(0, V2::new(105.0, 0.0), V2::ZERO, false);
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        assert!(
            (w.heat_of(0) - (0.5 + 1.0 / 30.0)).abs() < 1e-3,
            "{}",
            w.heat_of(0)
        );
    }
}
