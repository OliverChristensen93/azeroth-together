//! The arena's comings and goings: new shrines as old ones are used, pots near the heroes and a
//! stray power-up now and then; what a broken pot spills; health shards and power-ups taken from
//! the ground; and the party's power-ups running out.

use super::*;
use crate::interact::pick_kind;
use crate::powerups::{DETONATION_BOSS, DETONATION_RADIUS, HEALTH_SHARD};

/// How far from a hero a new shrine appears: out of sight. How often is the rules'
/// `shrine_every`.
const SHRINE_RING: (f32, f32) = (45.0, 75.0);
/// Seconds between new pots, how far from a hero they appear, and how far from every hero one
/// is let go, to come back nearer.
const POT_EVERY: f32 = 2.0;
const POT_RING: (f32, f32) = (12.0, 30.0);
const POT_RECYCLE: f32 = 90.0;
/// Seconds between stray power-ups, and how far from a hero they appear.
pub(super) const POWERUP_EVERY: f32 = 45.0;
const POWERUP_RING: (f32, f32) = (10.0, 22.0);
/// Seconds a health shard or power-up lies before it fades, and how near a hero takes it (before
/// [`Stat::Pickup`]).
pub(super) const PICKUP_LIFE: f32 = 30.0;
const PICKUP_REACH: f32 = 2.0;
/// How far from a broken pot its spill lands.
const SPILL: (f32, f32) = (1.0, 3.0);

impl World {
    /// Each step's upkeep of the arena: used interactables leave when their time is up and pots
    /// far from every hero go; the party's power-ups and whatever lies on the ground run down;
    /// and new shrines, pots and power-ups appear around the living heroes.
    pub(super) fn tend_arena(&mut self) {
        for it in &mut self.interactables {
            if let InteractState::Spent(t) = &mut it.state {
                *t -= DT;
            }
        }
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        self.interactables.retain(|i| {
            let gone = matches!(i.state, InteractState::Spent(t) if t <= 0.0);
            let far = i.kind == InteractableKind::Pot
                && i.ready()
                && !living.is_empty()
                && living.iter().all(|p| p.distance(i.pos) > POT_RECYCLE);
            !gone && !far
        });
        let buffs = self.buffs.len();
        for b in &mut self.buffs {
            b.1 -= DT;
        }
        self.buffs.retain(|b| b.1 > 0.0);
        if self.buffs.len() != buffs {
            for h in 0..self.heroes.len() {
                self.refresh(h);
            }
        }
        for p in &mut self.pickups {
            p.left -= DT;
        }
        self.pickups.retain(|p| p.left > 0.0);
        if living.is_empty() {
            return;
        }
        self.tend_camps();
        self.shrine_timer -= DT;
        if self.shrine_timer <= 0.0 {
            self.shrine_timer = self.content.rules.shrine_every;
            self.spawn_shrine(&living);
        }
        self.pot_timer -= DT;
        if self.pot_timer <= 0.0 {
            self.pot_timer = POT_EVERY;
            self.spawn_pot(&living);
        }
        self.powerup_timer -= DT;
        if self.powerup_timer <= 0.0 {
            self.powerup_timer = POWERUP_EVERY;
            let near = living[self.rng.index(living.len())];
            let at = self.free_spot(near, POWERUP_RING);
            let p = PowerUp::ALL[self.rng.index(PowerUp::ALL.len())];
            self.drop_pickup(at, PickupKind::PowerUp(p));
        }
    }

    /// The zone's [`crate::ZoneDef::elite_camps`], spread over the arena clear of the start and of
    /// the shrines, each to get its elite on the first step.
    pub(super) fn place_camps(&mut self) {
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let center = self.center();
        let ring = ((zone.leash * 0.3).clamp(8.0, 30.0), zone.leash - 5.0);
        for _ in 0..zone.elite_camps {
            let mut taken: Vec<V2> = self.interactables.iter().map(|i| i.pos).collect();
            taken.extend(self.camps.iter().map(|c| c.pos));
            let pos = spot(zone, center, ring, &taken, &mut self.rng);
            self.camps.push(Camp {
                pos,
                elite: None,
                respawn: 0.0,
            });
        }
    }

    /// Each camp's elite: a camp whose elite died counts down the rules' `elite_respawn`, then
    /// raises a new one of the run's strongest kind, which guards it.
    fn tend_camps(&mut self) {
        for c in 0..self.camps.len() {
            if let Some(id) = self.camps[c].elite {
                if self.enemy(id).is_some() {
                    continue;
                }
                self.camps[c].elite = None;
                self.camps[c].respawn = self.content.rules.elite_respawn;
            }
            self.camps[c].respawn -= DT;
            if self.camps[c].respawn > 0.0 {
                continue;
            }
            let (_, top) = self.tiers();
            let def = self.zone().enemy_ix[top];
            let pos = self.camps[c].pos;
            let id = self.spawn_enemy(def, pos, pos + V2::new(1.0, 0.0), true);
            if let Some(e) = self.enemies.last_mut() {
                e.home = Some(pos);
            }
            self.camps[c].elite = Some(id);
        }
    }

    /// A spot `ring` yards from `around`, clear of every interactable.
    fn free_spot(&mut self, around: V2, ring: (f32, f32)) -> V2 {
        let taken: Vec<V2> = self.interactables.iter().map(|i| i.pos).collect();
        let content = self.content.clone();
        spot(&content.zones[self.zone], around, ring, &taken, &mut self.rng)
    }

    /// A new shrine, out of sight of a living hero, if fewer than the zone's
    /// [`crate::ZoneDef::shrines_at_once`] stand.
    fn spawn_shrine(&mut self, living: &[V2]) {
        let standing: Vec<InteractableKind> = self
            .interactables
            .iter()
            .filter(|i| i.live() && !matches!(i.kind, InteractableKind::Pot | InteractableKind::Cache))
            .map(|i| i.kind)
            .collect();
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        if standing.len() as u32 >= zone.shrines_at_once {
            return;
        }
        let Some(kind) = pick_kind(zone, &standing, &mut self.rng) else {
            return;
        };
        let near = living[self.rng.index(living.len())];
        let at = self.free_spot(near, SHRINE_RING);
        self.place(kind, at);
    }

    /// A new pot near a living hero, if fewer than the zone's [`crate::ZoneDef::pots`] stand.
    fn spawn_pot(&mut self, living: &[V2]) {
        let pots = self
            .interactables
            .iter()
            .filter(|i| i.kind == InteractableKind::Pot && i.ready())
            .count() as u32;
        if pots >= self.zone().pots {
            return;
        }
        let near = living[self.rng.index(living.len())];
        let at = self.free_spot(near, POT_RING);
        self.place(InteractableKind::Pot, at);
    }

    /// A pot at `pos` broke: a few coins, a few gems, a health shard or a power-up spill around it.
    pub(super) fn break_pot(&mut self, pos: V2) {
        let minutes = self.elapsed / 60.0;
        let roll = self.rng.f32();
        let spill = |w: &mut World| {
            let angle = w.rng.range(0.0, std::f32::consts::TAU);
            pos + V2::from_angle(angle, w.rng.range(SPILL.0, SPILL.1))
        };
        if roll < 0.4 {
            for _ in 0..2 + self.rng.index(3) {
                let at = spill(self);
                self.drop_gem(at, 0, (2.0 + minutes).ceil() as u32);
            }
        } else if roll < 0.7 {
            let xp = (xp_to_next(self.level) * 0.08).ceil().max(1.0) as u32;
            for _ in 0..2 + self.rng.index(2) {
                let at = spill(self);
                self.drop_gem(at, xp, 0);
            }
        } else if roll < 0.88 {
            let at = spill(self);
            self.drop_pickup(at, PickupKind::Health);
        } else {
            let at = spill(self);
            let p = PowerUp::ALL[self.rng.index(PowerUp::ALL.len())];
            self.drop_pickup(at, PickupKind::PowerUp(p));
        }
    }

    /// Experience and gold on the ground at `pos`, told to the clients.
    pub(super) fn drop_gem(&mut self, pos: V2, xp: u32, gold: u32) {
        let id = self.id();
        self.gems.push(Gem { id, pos, xp, gold });
        self.events.push(Event::GemDropped { id, pos, xp, gold });
    }

    /// A health shard or power-up on the ground at `pos` (a test run's showcase lays them too).
    pub fn drop_pickup(&mut self, pos: V2, kind: PickupKind) {
        let id = self.id();
        self.pickups.push(Pickup {
            id,
            pos,
            kind,
            left: PICKUP_LIFE,
        });
    }

    /// Health shards and power-ups within a living hero's reach ([`PICKUP_REACH`] and its
    /// [`Stat::Pickup`]) are theirs.
    pub(super) fn collect_pickups(&mut self) {
        let reach: Vec<(PlayerId, V2, f32)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos, PICKUP_REACH * (1.0 + h.stats.get(Stat::Pickup))))
            .collect();
        let mut taken = Vec::new();
        self.pickups.retain(
            |p| match reach.iter().find(|&&(_, at, r)| at.distance(p.pos) <= r) {
                Some(&(by, ..)) => {
                    taken.push((p.id, p.kind, by));
                    false
                }
                None => true,
            },
        );
        for (id, kind, by) in taken {
            self.events.push(Event::PickupTaken { id, kind, by });
            match kind {
                PickupKind::Health => {
                    if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == by) {
                        hero.hp = (hero.hp + hero.max_hp * HEALTH_SHARD).min(hero.max_hp);
                    }
                }
                PickupKind::PowerUp(p) => self.power_up(p, by),
            }
        }
    }

    /// `by` took power-up `p`: a lasting one works for the whole party (taken again, its time
    /// starts over); a Detonation goes off around `by`; a Magnet pulls everything on the ground
    /// to `by`.
    pub(super) fn power_up(&mut self, p: PowerUp, by: PlayerId) {
        if let Some(secs) = p.duration() {
            match self.buffs.iter_mut().find(|b| b.0 == p) {
                Some(b) => b.1 = b.1.max(secs),
                None => self.buffs.push((p, secs)),
            }
            for h in 0..self.heroes.len() {
                self.refresh(h);
            }
            return;
        }
        match p {
            PowerUp::Detonation => {
                let Some(at) = self.hero(by).map(|h| h.pos) else {
                    return;
                };
                let blast: Vec<(u32, f32)> = self
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0.0 && e.pos.distance(at) <= DETONATION_RADIUS)
                    .map(|e| (e.id, if e.boss { e.max_hp * DETONATION_BOSS } else { e.hp }))
                    .collect();
                let Some(ability) = self.content.ability("detonation") else {
                    return;
                };
                let visual = self.content.abilities[ability].visual;
                self.events.push(Event::Cast {
                    caster: by,
                    ability,
                    visual,
                    targets: blast.iter().map(|&(id, _)| Target::Enemy(id)).collect(),
                    dest: Some(at),
                    cooldown: 0.0,
                });
                for (id, damage) in blast {
                    self.damage_enemy(by, ability, id, damage, false);
                }
            }
            PowerUp::Magnet => {
                for gem in std::mem::take(&mut self.gems) {
                    self.take_gem(gem, by);
                }
            }
            _ => {}
        }
    }
}
