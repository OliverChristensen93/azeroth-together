//! Extraction mode's rules ([`crate::extraction`]): bounds and where things may stand, heat,
//! spawning by heat, hunters, letting far enemies go, and the exits and their flights. Each runs
//! only in an extraction run; an arena never reaches them.

use super::*;
use crate::extraction::{ExitState, ExtractionDef, ExtractionSettings, SkullDef};

/// Draws [`World::fitting`] makes before it settles for a spot that does not fit.
const FIT_TRIES: usize = 8;

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
    pub(super) fn map<'c>(&self, content: &'c Content) -> Option<&'c ExtractionDef> {
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

    /// A spot drawn by `draw` that fits the run: in the arena, the first drawn; in an extraction
    /// run, the first of a few inside the outline (with room to spare) and clear of the safe ring's
    /// margin, or failing that the last drawn, made to fit ([`World::placeable`]).
    pub(super) fn fitting(&mut self, mut draw: impl FnMut(&mut World) -> V2) -> V2 {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return draw(self);
        };
        let bounds = def.bounds();
        let mut at = V2::ZERO;
        for _ in 0..FIT_TRIES {
            at = draw(self);
            if bounds.inside(at, 3.0) && !def.in_margin(at) {
                return at;
            }
        }
        self.placeable(at)
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
    pub(super) fn party_heat(&self) -> f32 {
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
    pub(super) fn add_heat(&mut self, player: Option<PlayerId>, amount: f32) {
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
        self.send_hunters(def);
        self.recycle(def);
        self.let_far_shrines_go(def);
        self.tend_exits(def);
    }

    /// The floor rises with the night; heat climbs while anyone (shared) or the hero (not
    /// shared) is outside the safe ring, and cools while everyone (shared) or the hero is home.
    fn tend_heat(&mut self, def: &ExtractionDef) {
        let floor = def.floor(self.elapsed);
        let living: Vec<(PlayerId, bool, bool)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| {
                (
                    h.player,
                    def.in_safe(h.pos),
                    def.hot_zone_at(h.pos).is_some(),
                )
            })
            .collect();
        let Some(x) = self.run_mut() else {
            return;
        };
        x.floor = floor;
        let (outside, cool, hot) = (
            def.heat.outside * DT,
            def.safe.cool * DT,
            def.heat.hot_zone * DT,
        );
        if def.settings.heat_shared {
            if living.iter().any(|&(_, home, _)| !home) {
                x.heat += outside;
                if living.iter().any(|&(_, _, in_hot)| in_hot) {
                    x.heat += hot;
                }
            } else if !living.is_empty() {
                x.heat -= cool;
            }
            x.heat = x.heat.max(floor);
        } else {
            for &(p, home, in_hot) in &living {
                let h = &mut hero_heat(x, p).1;
                let next = if home {
                    *h - cool
                } else {
                    *h + outside + if in_hot { hot } else { 0.0 }
                };
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
    /// The horde, in an extraction run: around heroes outside the safe ring only, never within
    /// its margin or past the outline; as big, as high in the roster and as tough as the heat of
    /// the hero it comes for says.
    pub(super) fn spawn_in_extraction(&mut self) {
        self.spawn_timer -= DT;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = SPAWN_EVERY;
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        let outside: Vec<(PlayerId, V2, SkullDef)> = self
            .heroes
            .iter()
            .filter(|h| h.alive && !def.in_safe(h.pos))
            .map(|h| (h.player, h.pos, *def.skull(self.heat_of(h.player))))
            .collect();
        if outside.is_empty() {
            return;
        }
        let busiest = outside.iter().map(|o| o.2.spawns).fold(1.0, f32::max);
        let horde = 1.0 + self.difficulty.horde;
        let target =
            (target_alive(self.elapsed, outside.len()) as f32 * busiest * horde * def.horde_share)
                as usize;
        let target = target.min(def.max_enemies_per_hero as usize * outside.len());
        let alive = self
            .enemies
            .iter()
            .filter(|e| !e.elite && !e.boss && !e.is_wild())
            .count();
        if alive >= target {
            return;
        }
        let batch = ((target - alive) as f32 / 6.0).ceil().min(4.0) as usize;
        let (low, top) = self.tiers();
        let roster = self.zone().enemy_ix.clone();
        let bounds = def.bounds();
        for _ in 0..batch {
            let (_, near, skull) = outside[self.rng.index(outside.len())];
            let Some(pos) = self.spawn_spot(def, &bounds, near) else {
                continue;
            };
            let tier = (low + self.rng.index(top - low + 1) + skull.tier).min(roster.len() - 1);
            let elite = self.elapsed >= ELITES_FROM && self.rng.f32() < ELITE_CHANCE;
            let id = self.spawn_enemy(roster[tier], pos, near, elite);
            self.toughen(id, skull.toughness, 1.0);
        }
    }

    /// A spot on the spawn ring round `near`, inside the outline and clear of the safe ring's
    /// margin; `None` after a few tries.
    fn spawn_spot(&mut self, def: &ExtractionDef, bounds: &Bounds, near: V2) -> Option<V2> {
        for _ in 0..8 {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let at = near + V2::from_angle(angle, self.rng.range(SPAWN_RING.0, SPAWN_RING.1));
            if bounds.inside(at, 0.0) && !def.in_margin(at) {
                return Some(at);
            }
        }
        None
    }

    /// Enemy `id`'s health and damage times `by`, and its speed times `speed`.
    pub(super) fn toughen(&mut self, id: u32, by: f32, speed: f32) {
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
            e.hp *= by;
            e.max_hp *= by;
            e.damage *= by;
            e.speed *= speed;
        }
    }

    /// When the hunt timer has run out, a hunter pack for a hero outside whose skull sends them:
    /// the strongest enemies in play, faster than the rest, coming from the way home.
    fn send_hunters(&mut self, def: &ExtractionDef) {
        if let Some(x) = self.run_mut() {
            x.hunt -= DT;
            if x.hunt > 0.0 {
                return;
            }
        }
        let hunted: Vec<(PlayerId, V2, SkullDef)> = self
            .heroes
            .iter()
            .filter(|h| h.alive && !def.in_safe(h.pos))
            .map(|h| (h.player, h.pos, *def.skull(self.heat_of(h.player))))
            .filter(|(_, _, s)| s.hunters_every > 0.0)
            .collect();
        if hunted.is_empty() {
            return;
        }
        let (player, from, skull) = hunted[self.rng.index(hunted.len())];
        let roster = self.zone().enemy_ix.clone();
        let (_, top) = self.tiers();
        let kind = roster[(top + skull.tier).min(roster.len() - 1)];
        let pack = def.heat.hunters;
        let count = pack.pack.0 + self.rng.index((pack.pack.1 - pack.pack.0 + 1) as usize) as u32;
        let home = (def.safe_center() - from).normalize_or_zero();
        let half = pack.cone.to_radians() / 2.0;
        let bounds = def.bounds();
        for _ in 0..count {
            let (sin, cos) = self.rng.range(-half, half).sin_cos();
            let way = V2::new(home.x * cos - home.y * sin, home.x * sin + home.y * cos);
            let at = from + way * self.rng.range(pack.ring.0, pack.ring.1);
            if !bounds.inside(at, 0.0) || def.in_margin(at) {
                continue;
            }
            let id = self.spawn_enemy(kind, at, from, false);
            self.toughen(id, skull.toughness, pack.speed);
        }
        if let Some(x) = self.run_mut() {
            x.hunt = skull.hunters_every;
        }
        self.events.push(Event::HuntersCame { near: player });
    }

    /// A ready shrine, altar, chest or banner further than the map's reach from every living hero
    /// goes, making room for a new one near them. An earned cache stays where it was left.
    fn let_far_shrines_go(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        if living.is_empty() {
            return;
        }
        let reach = def.shrine_reach;
        self.interactables.retain(|i| {
            let far = living.iter().all(|p| p.distance(i.pos) > reach);
            !(far && i.ready() && i.kind != InteractableKind::Cache)
        });
    }
    /// Ordinary enemies further than the map's reach from every living hero are let go: a party
    /// spread over the forest drags no tail of stragglers. Elites, bosses, the forest's creatures
    /// and an ambush still waiting for its flight stay.
    fn recycle(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        if living.is_empty() {
            return;
        }
        let reach = def.recycle;
        let now = self.elapsed;
        let waiting: Vec<u32> = match self.run_mut() {
            Some(x) => {
                x.ambushers.retain(|&(_, until)| now < until);
                x.ambushers.iter().map(|&(id, _)| id).collect()
            }
            None => Vec::new(),
        };
        self.enemies.retain(|e| {
            e.elite
                || e.boss
                || e.is_wild()
                || waiting.contains(&e.id)
                || living.iter().any(|p| p.distance(e.pos) <= reach)
        });
    }

    /// Flights keep their exits' timetables: each lands `window` seconds before it departs and
    /// waits, heating the party (shared) or whoever waits in its ring; at departure it takes
    /// every living hero in its ring, and the next flight is due `every` seconds later.
    fn tend_exits(&mut self, def: &ExtractionDef) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        let now = self.elapsed;
        let mut events = Vec::new();
        let mut heat: Vec<(Option<PlayerId>, f32)> = Vec::new();
        let mut taken: Vec<(PlayerId, u8)> = Vec::new();
        // Each departure's ambush is rolled once.
        let unrolled: Vec<usize> = match &self.mode {
            Mode::Extraction(x) => (0..x.exits.len()).filter(|&i| !x.exits[i].rolled).collect(),
            Mode::Arena => return,
        };
        for i in unrolled {
            let ambush = self.rng.f32() < def.ambush.chance;
            if let Some(x) = self.run_mut() {
                x.exits[i].rolled = true;
                x.exits[i].ambush = ambush;
            }
        }
        let mut packs = Vec::new();
        let Some(x) = self.run_mut() else {
            return;
        };
        for (i, (exit, state)) in def.exits.iter().zip(x.exits.iter_mut()).enumerate() {
            let tag = i as u8;
            let lands = state.lands_in(now);
            if state.ambush && !state.warned && lands <= def.ambush.warn {
                state.warned = true;
                events.push(Event::AmbushWarned { exit: tag });
            }
            if state.ambush && !state.came && lands <= 0.0 {
                state.came = true;
                packs.push(i);
            }
            if lands > 0.0 {
                continue;
            }
            let at = V2::new(exit.pos[0], exit.pos[1]);
            let inside: Vec<PlayerId> = living
                .iter()
                .filter(|(_, p)| p.distance(at) <= exit.radius)
                .map(|&(p, _)| p)
                .collect();
            if !state.boarding {
                state.boarding = true;
                events.push(Event::FlightLanded { exit: tag });
            }
            let gain = def.heat.flight * DT;
            if def.settings.heat_shared {
                heat.push((None, gain));
            } else {
                heat.extend(inside.iter().map(|&p| (Some(p), gain)));
            }
            if now < state.departs {
                continue;
            }
            taken.extend(inside.iter().map(|&p| (p, tag)));
            events.push(Event::FlightDeparted { exit: tag });
            *state = ExitState {
                departs: state.departs + exit.every,
                window: state.window,
                ..ExitState::default()
            };
        }
        for exit in packs {
            self.send_ambush(def, exit);
        }
        for (who, amount) in heat {
            self.add_heat(who, amount);
        }
        for (player, exit) in taken {
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.alive = false;
                hero.extracted = true;
                hero.moving = false;
            }
            events.push(Event::HeroExtracted { player, exit });
        }
        self.events.extend(events);
    }

    /// A pack converging on exit `exit`'s flight from one side: the strongest enemies in play,
    /// more of them the hotter the party.
    fn send_ambush(&mut self, def: &ExtractionDef, exit: usize) {
        let at = def.exit_pos(exit);
        let skulls = def.skull_index(self.party_heat());
        let skull = def.heat.skulls[skulls];
        let (lo, hi) = def.ambush.pack;
        let base = lo + self.rng.index((hi - lo + 1) as usize) as u32;
        let count = (base as f32 * (1.0 + def.ambush.per_skull * skulls as f32)).round() as u32;
        let roster = self.zone().enemy_ix.clone();
        let (_, top) = self.tiers();
        let kind = roster[(top + skull.tier).min(roster.len() - 1)];
        let side = self.rng.range(0.0, std::f32::consts::TAU);
        let bounds = def.bounds();
        let departs = match &self.mode {
            Mode::Extraction(x) => x.exits[exit].departs,
            Mode::Arena => return,
        };
        for _ in 0..count {
            let angle = side + self.rng.range(-0.5, 0.5);
            let r = self.rng.range(def.ambush.ring.0, def.ambush.ring.1);
            let pos = at + V2::from_angle(angle, r);
            if !bounds.inside(pos, 0.0) || def.in_margin(pos) {
                continue;
            }
            let id = self.spawn_enemy(kind, pos, at, false);
            self.toughen(id, skull.toughness, 1.0);
            if let Some(x) = self.run_mut() {
                x.ambushers.push((id, departs));
            }
        }
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
pub(super) mod tests {
    use super::*;
    use crate::extraction::tests::test_content;
    use crate::extraction::HotZoneDef;

    /// An extraction run on the test map with `heroes` mages, settings as `settings` says.
    pub(crate) fn run(settings: &str, heroes: u8) -> World {
        let mut w = World::new(Arc::new(test_content(settings)), "goldshire", 7).unwrap();
        for p in 0..heroes {
            w.add_hero(p, "mage").unwrap();
        }
        w
    }

    /// No horde: only what a test puts there.
    pub(crate) fn hush(w: &mut World) {
        w.spawn_timer = f32::MAX;
    }

    pub(crate) fn at(w: &mut World, player: PlayerId, x: f32, y: f32) {
        w.heroes
            .iter_mut()
            .find(|h| h.player == player)
            .unwrap()
            .pos = V2::new(x, y);
    }

    pub(crate) fn secs(w: &mut World, s: f32) {
        for _ in 0..(s * TICK_HZ as f32).round() as usize {
            w.step();
        }
    }

    pub(crate) fn state(w: &World) -> &Extraction {
        match &w.mode {
            Mode::Extraction(x) => x,
            Mode::Arena => panic!("not an extraction run"),
        }
    }

    pub(crate) fn state_mut(w: &mut World) -> &mut Extraction {
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
        assert!(x.exits.iter().all(|e| !e.boarding));
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

    #[test]
    fn nothing_spawns_while_everyone_is_home_or_near_the_safe_ring() {
        let mut w = run("", 1);
        secs(&mut w, 10.0);
        assert!(w.enemies.is_empty(), "home: no horde");
        at(&mut w, 0, 60.0, 0.0);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..300 {
            w.step();
            for e in &w.enemies {
                // Seen a step after spawning, so up to one step's walk nearer.
                if seen.insert(e.id) {
                    assert!(e.pos.distance(V2::ZERO) >= 89.5, "{:?}", e.pos);
                }
            }
        }
        assert!(!seen.is_empty(), "outside: a horde");
    }

    #[test]
    fn more_skulls_bring_a_bigger_tougher_horde() {
        let mut cold = run("", 1);
        let mut hot = run("", 1);
        at(&mut cold, 0, 200.0, 0.0);
        at(&mut hot, 0, 200.0, 0.0);
        state_mut(&mut hot).heat = 900.0;
        secs(&mut cold, 3.0);
        secs(&mut hot, 3.0);
        assert!(hot.enemies.len() > cold.enemies.len());
        let wolf = hot.content().enemies[0].hp;
        assert!(hot.enemies.iter().all(|e| e.max_hp >= wolf * 1.7));
        assert!(cold.enemies.iter().all(|e| e.max_hp < wolf * 1.1));
    }

    #[test]
    fn hunters_come_between_a_hot_hero_and_home() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 200.0, 0.0);
        state_mut(&mut w).heat = 450.0;
        w.step();
        let hero = V2::new(200.0, 0.0);
        let hunters: Vec<&Enemy> = w.enemies.iter().filter(|e| e.speed > 5.5).collect();
        assert!((6..=10).contains(&hunters.len()), "{}", hunters.len());
        let cone = 30f32.to_radians().cos();
        for e in hunters {
            let to = e.pos - hero;
            let d = to.length();
            assert!((50.0..=70.0).contains(&d), "{d}");
            assert!(
                -to.x / d >= cone - 1e-3,
                "{:?} is not on the way home",
                e.pos
            );
        }
        assert!(w.drain_events().contains(&Event::HuntersCame { near: 0 }));
    }

    #[test]
    fn far_enemies_are_let_go_but_never_elites() {
        let mut w = run("", 1);
        hush(&mut w);
        let near = w.spawn_enemy(0, V2::new(50.0, 0.0), V2::ZERO, false);
        let far = w.spawn_enemy(0, V2::new(250.0, 0.0), V2::ZERO, false);
        let elite = w.spawn_enemy(0, V2::new(-250.0, 0.0), V2::ZERO, true);
        w.step();
        assert!(w.enemy(near).is_some());
        assert!(w.enemy(far).is_none());
        assert!(w.enemy(elite).is_some());
    }

    #[test]
    fn the_timed_boss_comes_only_when_asked_for() {
        for (settings, comes) in [("", false), ("timed_boss: true", true)] {
            let mut w = run(settings, 1);
            hush(&mut w);
            w.next_boss = 0.0;
            w.step();
            assert_eq!(w.enemies.iter().any(|e| e.boss), comes, "{settings}");
        }
    }

    #[test]
    fn an_extracted_hero_is_never_raised() {
        let mut w = run("", 2);
        hush(&mut w);
        let gone = w.heroes.iter_mut().find(|h| h.player == 0).unwrap();
        gone.alive = false;
        gone.extracted = true;
        secs(&mut w, 5.0);
        assert!(!w.hero(0).unwrap().alive);
    }

    #[test]
    fn an_extracted_hero_gets_no_cards_and_never_holds_the_run() {
        let mut w = run("", 2);
        hush(&mut w);
        let gone = w.heroes.iter_mut().find(|h| h.player == 0).unwrap();
        gone.alive = false;
        gone.extracted = true;
        w.xp = xp_to_next(w.level);
        w.step();
        assert!(w.hero(0).unwrap().offers.is_empty());
        assert!(!w.hero(1).unwrap().offers.is_empty());
        w.set_input(
            1,
            PlayerInput {
                pick: Some(0),
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused());
    }

    /// Inside the outline with room to spare, and clear of the safe ring's margin.
    fn fits(w: &World, p: V2) -> bool {
        let def = &w.content().extractions[0];
        def.bounds().inside(p, 3.0) && !def.in_margin(p)
    }

    #[test]
    fn the_first_shrines_stand_near_home_inside_the_outline() {
        let content = Arc::new(test_content(""));
        for seed in 1..20 {
            let w = World::new(content.clone(), "goldshire", seed).unwrap();
            for i in &w.interactables {
                assert!(fits(&w, i.pos), "seed {seed}: {:?}", i.pos);
                assert!(
                    i.pos.distance(V2::ZERO) <= 250.0 + 1e-3,
                    "seed {seed}: {:?}",
                    i.pos
                );
            }
        }
    }

    #[test]
    fn shrines_and_power_ups_near_a_hero_by_the_edge_stay_inside() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 250.0, 0.0);
        for _ in 0..40 {
            w.interactables.clear();
            w.pickups.clear();
            w.shrine_timer = 0.0;
            w.powerup_timer = 0.0;
            w.step();
            let shrines = w.interactables.iter().map(|i| i.pos);
            let pickups = w.pickups.iter().map(|p| p.pos);
            for p in shrines.chain(pickups) {
                assert!(fits(&w, p), "{p:?}");
            }
        }
    }

    #[test]
    fn a_banner_is_carried_to_a_spot_inside_the_outline() {
        let content = Arc::new(test_content(""));
        for seed in 1..12 {
            let mut w = World::new(content.clone(), "goldshire", seed).unwrap();
            w.add_hero(0, "mage").unwrap();
            hush(&mut w);
            w.interactables.clear();
            at(&mut w, 0, 250.0, 0.0);
            let id = w.place(InteractableKind::WarBanner, V2::new(250.0, 0.0));
            let mut to = None;
            for _ in 0..600 {
                w.step();
                if let Some(InteractState::Escort { to: dest, .. }) =
                    w.interactable(id).map(|i| i.state.clone())
                {
                    to = Some(dest);
                    break;
                }
            }
            let to = to.expect("the banner was taken up");
            assert!(fits(&w, to), "seed {seed}: {to:?}");
        }
    }

    #[test]
    fn far_ready_shrines_go_but_caches_stay() {
        let mut w = run("", 1);
        hush(&mut w);
        w.interactables.clear();
        let far = w.place(InteractableKind::TitanShrine, V2::new(-290.0, -290.0));
        let near = w.place(InteractableKind::TitanShrine, V2::new(150.0, 0.0));
        let cache = w.place(InteractableKind::Cache, V2::new(290.0, 290.0));
        w.step();
        assert!(w.interactable(far).is_none(), "far from everyone: gone");
        assert!(w.interactable(near).is_some());
        assert!(w.interactable(cache).is_some(), "an earned cache stays");
    }

    #[test]
    fn the_horde_keeps_to_its_cap_however_hot() {
        let mut content = test_content("");
        content.extractions[0].max_enemies_per_hero = 20;
        let mut w = World::new(Arc::new(content), "goldshire", 3).unwrap();
        for p in 0..4 {
            w.add_hero(p, "mage").unwrap();
        }
        for (p, (x, y)) in [
            (200.0, 200.0),
            (200.0, -200.0),
            (-200.0, 200.0),
            (-200.0, -200.0),
        ]
        .into_iter()
        .enumerate()
        {
            at(&mut w, p as PlayerId, x, y);
        }
        state_mut(&mut w).heat = 1000.0;
        state_mut(&mut w).hunt = f32::MAX;
        for _ in 0..300 {
            w.step();
            let horde = w.enemies.iter().filter(|e| !e.elite && !e.boss).count();
            assert!(horde <= 80, "{horde}");
        }
    }

    #[test]
    fn a_flight_lands_on_its_timetable_and_departs_with_whoever_is_in_its_ring() {
        let mut w = run("", 2);
        hush(&mut w);
        at(&mut w, 0, 200.0, 0.0);
        // East Gate: first departure at 60, on the ground for 30 before it.
        w.elapsed = 29.0;
        secs(&mut w, 0.5);
        assert!(!state(&w).exits[0].boarding);
        secs(&mut w, 1.0);
        assert!(state(&w).exits[0].boarding);
        assert!(w.drain_events().contains(&Event::FlightLanded { exit: 0 }));
        secs(&mut w, 29.0);
        assert!(w.hero(0).unwrap().alive, "not yet");
        secs(&mut w, 1.0);
        let (out, home) = (w.hero(0).unwrap(), w.hero(1).unwrap());
        assert!(out.extracted && !out.alive);
        assert!(home.alive && !home.extracted);
        assert!(!w.game_over());
        let x = state(&w);
        assert!(!x.exits[0].boarding);
        assert!((x.exits[0].departs - 120.0).abs() < 1e-3, "the next flight");
        let events = w.drain_events();
        assert!(events.contains(&Event::FlightDeparted { exit: 0 }));
        assert!(events.contains(&Event::HeroExtracted { player: 0, exit: 0 }));
    }

    #[test]
    fn only_heroes_in_the_ring_at_departure_are_taken() {
        let mut w = run("", 2);
        hush(&mut w);
        w.elapsed = 45.0;
        at(&mut w, 0, 200.0, 0.0);
        at(&mut w, 1, 100.0, 0.0);
        secs(&mut w, 10.0);
        // The first walks off; the second arrives just in time.
        at(&mut w, 0, 100.0, 0.0);
        at(&mut w, 1, 201.0, 2.0);
        secs(&mut w, 6.0);
        assert!(!w.hero(0).unwrap().extracted);
        assert!(w.hero(1).unwrap().extracted);
    }

    #[test]
    fn a_boarding_flight_heats_the_party() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        w.elapsed = 30.0;
        secs(&mut w, 10.0);
        // 1 a second for being outside and 4 for the flight on the ground.
        assert!((w.heat_of(0) - 50.0).abs() < 0.5, "{}", w.heat_of(0));
    }

    #[test]
    fn the_last_one_out_ends_the_run_and_leaves_the_fallen() {
        let mut w = run("", 2);
        hush(&mut w);
        w.elapsed = 55.0;
        at(&mut w, 0, 200.0, 0.0);
        let fallen = w.heroes.iter_mut().find(|h| h.player == 1).unwrap();
        fallen.alive = false;
        fallen.hp = 0.0;
        secs(&mut w, 6.0);
        assert!(w.hero(0).unwrap().extracted);
        assert!(!w.hero(1).unwrap().extracted, "the fallen are left behind");
        assert!(w.game_over());
    }

    #[test]
    fn an_exit_departs_at_the_time_its_state_carries() {
        let mut w = run("", 1);
        hush(&mut w);
        assert_eq!(state(&w).exits[0].departs, 60.0);
        at(&mut w, 0, 200.0, 0.0);
        state_mut(&mut w).exits[0].departs = 31.0;
        secs(&mut w, 32.0);
        assert!(w.hero(0).unwrap().extracted);
    }

    #[test]
    fn an_ambush_is_warned_of_then_comes_from_one_side_when_the_flight_lands() {
        let mut content = test_content("");
        content.extractions[0].ambush.chance = 1.0;
        let mut w = World::new(Arc::new(content), "goldshire", 5).unwrap();
        w.add_hero(0, "mage").unwrap();
        hush(&mut w);
        // The hero waits near the gate (an ambush far from every hero is let go like any
        // straggler), casting nothing.
        at(&mut w, 0, 150.0, 0.0);
        w.heroes[0].abilities.clear();
        // East Gate lands at 30; the warning comes 20 before.
        w.elapsed = 9.0;
        secs(&mut w, 2.0);
        assert!(w.drain_events().contains(&Event::AmbushWarned { exit: 0 }));
        assert!(w.enemies.is_empty(), "warned, not come");
        secs(&mut w, 19.5);
        let gate = V2::new(200.0, 0.0);
        assert!(w.enemies.len() >= 6, "{}", w.enemies.len());
        for e in &w.enemies {
            let d = e.pos.distance(gate);
            assert!((35.0..=61.0).contains(&d), "{d}");
        }
    }

    #[test]
    fn an_ambush_waits_at_its_flight_for_a_party_still_far_off_until_it_departs() {
        let mut content = test_content("");
        content.extractions[0].ambush.chance = 1.0;
        let mut w = World::new(Arc::new(content), "goldshire", 5).unwrap();
        w.add_hero(0, "mage").unwrap();
        hush(&mut w);
        // The hero hangs back 220 yd from the East Gate, out of the recycle's reach.
        at(&mut w, 0, -20.0, 0.0);
        w.heroes[0].abilities.clear();
        w.elapsed = 29.0;
        secs(&mut w, 2.0);
        assert!(w.enemies.len() >= 6, "the pack came: {}", w.enemies.len());
        secs(&mut w, 25.0);
        assert!(w.enemies.len() >= 6, "still waiting while boarding: {}", w.enemies.len());
        // Departed at 60: from then on it is a straggler like any other.
        assert!(!state(&w).ambushers.is_empty());
        secs(&mut w, 6.0);
        assert!(state(&w).ambushers.is_empty());
    }

    #[test]
    fn a_guest_counts_down_to_the_landing_the_host_tuned() {
        let base = test_content("");
        let tuned = base.tuned(&crate::content::Tuning {
            flight: 2.0,
            ..Default::default()
        });
        let mut host = World::new(Arc::new(tuned.clone()), "goldshire", 7).unwrap();
        host.add_hero(0, "mage").unwrap();
        hush(&mut host);
        secs(&mut host, 1.0);
        // The guest plays the content as shipped; the host's tuning reaches it only by snapshot.
        let mut guest = World::new(Arc::new(base), "goldshire", 7).unwrap();
        guest.add_hero(0, "mage").unwrap();
        guest.apply_snapshot(host.snapshot(), host.drain_events());
        let on_host = state(&host).exits[0].lands_in(host.elapsed);
        let on_guest = state(&guest).exits[0].lands_in(guest.elapsed);
        assert!((on_host - on_guest).abs() < 0.01, "{on_host} vs {on_guest}");
    }

    #[test]
    fn without_a_chance_no_ambush_comes() {
        let mut w = run("", 1);
        hush(&mut w);
        w.elapsed = 9.0;
        secs(&mut w, 22.0);
        assert!(w.enemies.is_empty());
        assert!(!w
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::AmbushWarned { .. })));
    }

    #[test]
    fn heat_climbs_faster_in_a_hot_zone() {
        let mut c = test_content("");
        c.extractions[0].heat.hot_zone = 3.0;
        c.extractions[0].hot_zones.push(HotZoneDef {
            name: "Den".into(),
            center: [150.0, 0.0],
            radius: 30.0,
        });
        let mut w = World::new(Arc::new(c), "goldshire", 1).unwrap();
        w.add_hero(0, "mage").unwrap();
        hush(&mut w);
        at(&mut w, 0, 150.0, 0.0);
        secs(&mut w, 2.0);
        // 1 a second outside and 3 in the hot zone.
        assert!((w.heat_of(0) - 8.0).abs() < 0.1, "{}", w.heat_of(0));
    }

    #[test]
    fn the_horde_share_shrinks_the_horde() {
        let horde = |share: f32| {
            let mut c = test_content("");
            c.extractions[0].horde_share = share;
            let mut w = World::new(Arc::new(c), "goldshire", 4).unwrap();
            w.add_hero(0, "mage").unwrap();
            at(&mut w, 0, 0.0, 200.0); // off the exits, which would fly a hero out at 300 s
            w.elapsed = 300.0;
            secs(&mut w, 3.0);
            w.enemies.len()
        };
        let (full, half) = (horde(1.0), horde(0.5));
        assert!(half * 10 < full * 7, "{half} of {full}");
    }
}
