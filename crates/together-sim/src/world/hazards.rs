//! Elites' and bosses' mechanics ([`Mechanic`]) as the world runs them: each elite or boss
//! starts what its creature's mechanics allow when it is ready, marks the ground with a
//! [`Hazard`] (an [`Event::Telegraph`] for the clients), and a moment later the hazard goes off
//! on whoever is still inside. A charge, slam or leap needs its body: freezing or stunning the
//! elite while it winds up calls it off.

use super::*;
use crate::mechanics::{Mechanic, Shape};

/// A hazard's damage, in swings of the enemy that made it.
const CHARGE_HITS: f32 = 1.5;
const SLAM_HITS: f32 = 1.5;
const LEAP_HITS: f32 = 1.5;
const CASTER_HITS: f32 = 1.0;
const PATCH_HITS: f32 = 1.0;
/// Seconds of warning each gives.
const CHARGE_WINDUP: f32 = 1.0;
const SLAM_WINDUP: f32 = 1.2;
const LEAP_WINDUP: f32 = 1.1;
const CASTER_WINDUP: f32 = 1.2;
const PATCH_WINDUP: f32 = 1.4;
const GRIP_WINDUP: f32 = 2.5;
/// Sizes, in yards.
const CHARGE_WIDTH: f32 = 2.4;
const CHARGE_REACH: f32 = 20.0;
const SLAM_RADIUS: f32 = 6.0;
/// How far from a slam's centre a hero inside lands.
const SLAM_THROW: f32 = 10.0;
const LEAP_RADIUS: f32 = 3.5;
const CASTER_RADIUS: f32 = 2.5;
const PATCH_RADIUS: f32 = 2.5;
const GRIP_RADIUS: f32 = 6.0;
/// Patches around each hero: one under it, the rest scattered with gaps between.
const PATCHES: usize = 5;
/// Yards a second of a charge and a leap.
const CHARGE_SPEED: f32 = 28.0;
const LEAP_SPEED: f32 = 40.0;
/// A healer heals the enemies within this reach by this share of their health.
const HEAL_REACH: f32 = 9.0;
const HEAL_SHARE: f32 = 0.08;
/// Enemies a summoner calls each time.
const SUMMONS: usize = 2;
/// The share of an enemy's health a boss's patch burns from it: kiting trash through the fire
/// is a tactic.
const PATCH_BURNS_ENEMIES: f32 = 0.2;
/// Things to run to: how long a spore's bonus lasts and how much crit it gives; a bomb's fuse once
/// picked up, its reach, what it deals (a share of each enemy's health, more for ordinary ones)
/// and the share of the carrier's health it costs (never the last point); and a held circle's
/// share of the boss's health each second.
pub(super) const SPORE_CRIT: f32 = 0.5;
const SPORE_SECS: f32 = 10.0;
const BOMB_FUSE: f32 = 4.0;
const BOMB_REACH: f32 = 8.0;
const BOMB_BOSS_SHARE: f32 = 0.06;
const BOMB_SHARE: f32 = 0.5;
const BOMB_SELF: f32 = 0.1;
const CIRCLE_SHARE: f32 = 0.02;
/// How long each waits on the ground, and its size.
const SPORE_LIFE: f32 = 12.0;
const BOMB_LIFE: f32 = 15.0;
const CIRCLE_LIFE: f32 = 15.0;
const SPORE_RADIUS: f32 = 2.5;
const BOMB_RADIUS: f32 = 1.5;
const CIRCLE_RADIUS: f32 = 3.0;
/// Visuals: the spore taken (Holy Nova's glow), the bomb's blast (Arcane Explosion's), the
/// circle's bite on the boss (Holy Fire).
const SPORE_VISUAL: u32 = 15237;
const BOMB_VISUAL: u32 = 1449;
const CIRCLE_VISUAL: u32 = 14914;
/// An eruption (Volcanic): Flamestrike's pillar.
const VOLCANO_VISUAL: u32 = 2120;
/// A lure's pulse on the ground: Rejuvenation's green glow.
const LURE_PULSE: u32 = 774;

/// A hazard no enemy made (an affix's): it is not called off when an enemy dies.
pub(super) const NO_OWNER: u32 = u32::MAX;

/// A caster stops coming closer at this distance.
pub(super) const CASTER_KEEP: f32 = 12.0;
/// A frenzied enemy's speed, and its swings' cooldown.
pub(super) const FRENZY_SPEED: f32 = 1.5;
pub(super) const FRENZY_SWING: f32 = 0.8;

/// Spell visuals the mechanics play: Charge, Thunder Clap, Fireball, Rain of Fire, Flash Heal,
/// Summon Imp, Death Coil.
const CHARGE_VISUAL: u32 = 100;
const SLAM_VISUAL: u32 = 6343;
const CASTER_VISUAL: u32 = 133;
const PATCH_VISUAL: u32 = 5740;
const HEAL_VISUAL: u32 = 2061;
const SUMMON_VISUAL: u32 = 688;
const GRIP_VISUAL: u32 = 6789;

/// What an elite or boss is doing about its mechanics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Act {
    /// Seconds to each of its creature's mechanics' next use, filled at its first step.
    cds: Vec<f32>,
    /// Winding up: it stands still while its telegraph is up.
    pub(super) windup: f32,
    /// A charge or leap under way: where to, and yards a second.
    pub(super) dash: Option<(V2, f32)>,
    /// A Spiteful ghost: seconds before it fades.
    pub(super) expires: Option<f32>,
    /// Bolstering stacks taken.
    pub(super) bolster: u8,
}

/// Ground marked by an elite or boss, going off when `left` runs out.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Hazard {
    id: u32,
    kind: Mechanic,
    pos: V2,
    shape: Shape,
    left: f32,
    /// The enemy that made it: without it, it is called off.
    from: u32,
    damage: f32,
}

/// Whether `m` is worth starting with the nearest hero `dist` yards away.
fn usable(m: Mechanic, dist: f32) -> bool {
    match m {
        Mechanic::Charge => (5.0..18.0).contains(&dist),
        Mechanic::Caster => dist < 22.0,
        Mechanic::Slam => dist < 5.5,
        Mechanic::Leap => (9.0..25.0).contains(&dist),
        Mechanic::Frenzy => false,
        Mechanic::Healer => true,
        Mechanic::Summoner
        | Mechanic::Patches
        | Mechanic::DeathGrip
        | Mechanic::Spores
        | Mechanic::Bomb
        | Mechanic::Circle => dist < 40.0,
        Mechanic::Sanguine | Mechanic::Volcanic => false,
    }
}

impl World {
    /// Each step: telegraphs count down and go off, and every ready elite or boss starts what
    /// its mechanics allow.
    pub(super) fn tend_mechanics(&mut self) {
        self.tend_hazards();
        self.tend_lures();
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        if living.is_empty() {
            return;
        }
        let content = self.content.clone();
        for i in 0..self.enemies.len() {
            let e = &mut self.enemies[i];
            let mechanics = &content.enemies[e.def].mechanics;
            if !(e.elite || e.boss) || mechanics.is_empty() {
                continue;
            }
            if e.act.cds.len() != mechanics.len() {
                e.act.cds = mechanics.iter().map(|m| m.cooldown().1).collect();
            }
            for cd in &mut e.act.cds {
                *cd -= DT;
            }
            let (_, target) = living
                .iter()
                .copied()
                .min_by(|a, b| e.pos.distance(a.1).total_cmp(&e.pos.distance(b.1)))
                .expect("living is not empty");
            let dist = e.pos.distance(target);
            let guarding = e.home.is_some() && dist > ELITE_AGGRO;
            if e.act.windup > 0.0 || e.act.dash.is_some() || e.status.held() || guarding {
                continue;
            }
            let ready = mechanics
                .iter()
                .enumerate()
                .find(|&(k, &m)| e.act.cds[k] <= 0.0 && usable(m, dist));
            if let Some((k, &m)) = ready {
                e.act.cds[k] = m.cooldown().0;
                self.start(i, m, target, &living);
            }
        }
    }

    /// Enemy `i` starts mechanic `m` against the nearest hero, at `target`.
    fn start(&mut self, i: usize, m: Mechanic, target: V2, living: &[(PlayerId, V2)]) {
        let (from, pos, damage) = {
            let e = &self.enemies[i];
            (e.id, e.pos, e.damage)
        };
        let toward = (target - pos).normalize_or_zero();
        match m {
            Mechanic::Charge => {
                let reach = (pos.distance(target) + 4.0).min(CHARGE_REACH);
                let shape = Shape::Line {
                    to: pos + toward * reach,
                    width: CHARGE_WIDTH,
                };
                self.enemies[i].facing = toward;
                self.enemies[i].act.windup = CHARGE_WINDUP;
                self.mark(m, pos, shape, CHARGE_WINDUP, from, damage * CHARGE_HITS);
            }
            Mechanic::Slam => {
                let shape = Shape::Circle {
                    radius: SLAM_RADIUS,
                };
                self.enemies[i].act.windup = SLAM_WINDUP;
                self.mark(m, pos, shape, SLAM_WINDUP, from, damage * SLAM_HITS);
            }
            Mechanic::Leap => {
                let shape = Shape::Circle {
                    radius: LEAP_RADIUS,
                };
                self.enemies[i].facing = toward;
                self.enemies[i].act.windup = LEAP_WINDUP;
                self.mark(m, target, shape, LEAP_WINDUP, from, damage * LEAP_HITS);
            }
            Mechanic::Caster => {
                let shape = Shape::Circle {
                    radius: CASTER_RADIUS,
                };
                self.mark(m, target, shape, CASTER_WINDUP, from, damage * CASTER_HITS);
            }
            Mechanic::Frenzy | Mechanic::Sanguine | Mechanic::Volcanic => {}
            Mechanic::Healer => {
                let mut healed = Vec::new();
                for e in self.enemies.iter_mut().filter(|e| {
                    e.id != from && e.hp < e.max_hp && e.pos.distance(pos) <= HEAL_REACH
                }) {
                    e.hp = (e.hp + e.max_hp * HEAL_SHARE).min(e.max_hp);
                    healed.push(Target::Enemy(e.id));
                }
                if !healed.is_empty() {
                    self.events.push(Event::EnemyCast {
                        enemy: from,
                        visual: HEAL_VISUAL,
                        targets: healed,
                        dest: None,
                    });
                }
            }
            Mechanic::Summoner => {
                let def = self.zone().enemy_ix[self.tiers().0];
                for _ in 0..SUMMONS {
                    let at = pos + V2::from_angle(self.rng.range(0.0, std::f32::consts::TAU), 2.5);
                    self.spawn_enemy(def, at, target, false);
                }
                self.events.push(Event::EnemyCast {
                    enemy: from,
                    visual: SUMMON_VISUAL,
                    targets: Vec::new(),
                    dest: None,
                });
            }
            Mechanic::Patches => {
                let shape = Shape::Circle {
                    radius: PATCH_RADIUS,
                };
                for &(_, at) in living {
                    let mut spots = vec![at];
                    let mut tries = 0;
                    while spots.len() < PATCHES && tries < 40 {
                        tries += 1;
                        let angle = self.rng.range(0.0, std::f32::consts::TAU);
                        let spot = at + V2::from_angle(angle, self.rng.range(3.0, 8.0));
                        // Gaps between the patches, so there is always a way through.
                        if spots
                            .iter()
                            .all(|s| s.distance(spot) > PATCH_RADIUS * 2.0 + 1.5)
                        {
                            spots.push(spot);
                        }
                    }
                    for spot in spots {
                        self.mark(m, spot, shape, PATCH_WINDUP, from, damage * PATCH_HITS);
                    }
                }
            }
            Mechanic::Spores | Mechanic::Bomb => {
                let (_, near) = living[self.rng.index(living.len())];
                let angle = self.rng.range(0.0, std::f32::consts::TAU);
                let at = near + V2::from_angle(angle, self.rng.range(6.0, 12.0));
                let (radius, life) = if m == Mechanic::Spores {
                    (SPORE_RADIUS, SPORE_LIFE)
                } else {
                    (BOMB_RADIUS, BOMB_LIFE)
                };
                self.mark(m, at, Shape::Circle { radius }, life, from, 0.0);
            }
            Mechanic::Circle => {
                let angle = self.rng.range(0.0, std::f32::consts::TAU);
                let at = pos + V2::from_angle(angle, self.rng.range(8.0, 14.0));
                let shape = Shape::Circle {
                    radius: CIRCLE_RADIUS,
                };
                self.mark(m, at, shape, CIRCLE_LIFE, from, 0.0);
            }
            Mechanic::DeathGrip => {
                let (_, at) = living[self.rng.index(living.len())];
                let shape = Shape::Circle {
                    radius: GRIP_RADIUS,
                };
                self.mark(m, at, shape, GRIP_WINDUP, from, 0.0);
            }
        }
    }

    /// Mark ground for mechanic `kind`, going off in `secs`.
    pub(super) fn mark(
        &mut self,
        kind: Mechanic,
        pos: V2,
        shape: Shape,
        secs: f32,
        from: u32,
        damage: f32,
    ) {
        let id = self.id();
        self.hazards.push(Hazard {
            id,
            kind,
            pos,
            shape,
            left: secs,
            from,
            damage,
        });
        self.events.push(Event::Telegraph {
            id,
            kind,
            pos,
            shape,
            secs,
        });
    }

    /// Telegraphs count down; those whose enemy died (or, for a charge, slam or leap, was frozen
    /// or stunned) are called off, and those that run out go off.
    fn tend_hazards(&mut self) {
        let mut fired = Vec::new();
        let mut i = 0;
        while i < self.hazards.len() {
            self.hazards[i].left -= DT;
            let (from, kind, left) = {
                let h = &self.hazards[i];
                (h.from, h.kind, h.left)
            };
            let owner = self.enemies.iter().position(|e| e.id == from);
            let body = matches!(kind, Mechanic::Charge | Mechanic::Slam | Mechanic::Leap);
            let held = owner.is_some_and(|o| self.enemies[o].status.held());
            if (owner.is_none() && from != NO_OWNER) || (body && held) {
                if let Some(o) = owner {
                    self.enemies[o].act.windup = 0.0;
                }
                let h = self.hazards.remove(i);
                self.events.push(Event::TelegraphEnd { id: h.id });
                continue;
            }
            if left <= 0.0 {
                fired.push(self.hazards.remove(i));
                continue;
            }
            i += 1;
        }
        for h in fired {
            self.events.push(Event::TelegraphEnd { id: h.id });
            self.go_off(h);
        }
    }

    /// A telegraph goes off on whoever is still inside.
    fn go_off(&mut self, h: Hazard) {
        let owner = self.enemies.iter().position(|e| e.id == h.from);
        let inside: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|x| x.alive && h.shape.contains(h.pos, x.pos, HERO_RADIUS))
            .map(|x| (x.player, x.pos))
            .collect();
        if h.damage > 0.0 {
            for &(player, _) in &inside {
                let amount = self.hurt_hero_from(player, h.damage, Some(h.from));
                if amount > 0.0 {
                    self.events.push(Event::Damage {
                        target: Target::Hero(player),
                        amount,
                        crit: false,
                    });
                }
            }
        }
        let cast = |visual: u32, dest: Option<V2>| Event::EnemyCast {
            enemy: h.from,
            visual,
            targets: Vec::new(),
            dest,
        };
        match h.kind {
            Mechanic::Charge | Mechanic::Leap => {
                let (to, speed, visual) = match h.shape {
                    Shape::Line { to, .. } => (to, CHARGE_SPEED, Some(CHARGE_VISUAL)),
                    Shape::Circle { .. } => (h.pos, LEAP_SPEED, None),
                };
                if let Some(o) = owner {
                    let e = &mut self.enemies[o];
                    e.act.windup = 0.0;
                    e.act.dash = Some((to, speed));
                }
                if let Some(visual) = visual {
                    self.events.push(cast(visual, None));
                }
            }
            Mechanic::Slam => {
                if let Some(o) = owner {
                    self.enemies[o].act.windup = 0.0;
                }
                for &(player, at) in &inside {
                    let away = (at - h.pos).normalize_or_zero();
                    let away = if away == V2::ZERO {
                        V2::new(1.0, 0.0)
                    } else {
                        away
                    };
                    self.shove(player, h.pos + away * SLAM_THROW);
                }
                self.events.push(cast(SLAM_VISUAL, Some(h.pos)));
            }
            Mechanic::Caster => self.events.push(cast(CASTER_VISUAL, Some(h.pos))),
            Mechanic::Patches => {
                for e in self
                    .enemies
                    .iter_mut()
                    .filter(|e| !e.boss && h.shape.contains(h.pos, e.pos, e.radius))
                {
                    e.hp -= e.max_hp * PATCH_BURNS_ENEMIES;
                }
                self.events.push(cast(PATCH_VISUAL, Some(h.pos)));
            }
            Mechanic::DeathGrip => {
                if let Some(o) = owner {
                    let at = self.enemies[o].pos;
                    for &(player, pos) in &inside {
                        let side = (pos - at).normalize_or_zero();
                        self.shove(player, at + side * 2.5);
                    }
                }
                self.events.push(cast(GRIP_VISUAL, Some(h.pos)));
            }
            // A lure that waited out its time just goes.
            Mechanic::Volcanic => self.events.push(cast(VOLCANO_VISUAL, Some(h.pos))),
            Mechanic::Frenzy
            | Mechanic::Healer
            | Mechanic::Summoner
            | Mechanic::Spores
            | Mechanic::Bomb
            | Mechanic::Circle
            | Mechanic::Sanguine => {}
        }
    }

    /// Things to run to: a spore or a bomb a hero walks into is taken, a held circle bites its
    /// boss, and a carried bomb's fuse runs down to its blast.
    fn tend_lures(&mut self) {
        let mut taken = Vec::new();
        let mut bites = Vec::new();
        for (i, h) in self.hazards.iter().enumerate() {
            if !h.kind.is_lure() {
                continue;
            }
            let inside = self
                .heroes
                .iter()
                .position(|x| x.alive && h.shape.contains(h.pos, x.pos, HERO_RADIUS));
            match (h.kind, inside) {
                (Mechanic::Circle, Some(_)) => bites.push(h.from),
                (Mechanic::Spores | Mechanic::Bomb, Some(hero)) => taken.push((i, hero)),
                _ => {}
            }
        }
        // Things to run to pulse, so they stand out from the shrines' runes.
        if self.tick % u64::from(TICK_HZ * 3 / 2) == 0 {
            let pulses: Vec<Event> = self
                .hazards
                .iter()
                .filter(|h| h.kind.is_lure())
                .map(|h| Event::EnemyCast {
                    enemy: h.from,
                    visual: LURE_PULSE,
                    targets: Vec::new(),
                    dest: Some(h.pos),
                })
                .collect();
            self.events.extend(pulses);
        }
        for from in bites {
            if let Some(boss) = self.enemies.iter_mut().find(|e| e.id == from) {
                let amount = boss.max_hp * CIRCLE_SHARE * DT;
                boss.hp -= amount;
                // A bite a second is enough to see.
                if (self.tick % u64::from(TICK_HZ)) == 0 {
                    self.events.push(Event::EnemyCast {
                        enemy: from,
                        visual: CIRCLE_VISUAL,
                        targets: vec![Target::Enemy(from)],
                        dest: None,
                    });
                }
            }
        }
        // Taken last first, so the indices still hold.
        for &(i, hero) in taken.iter().rev() {
            let lure = self.hazards.remove(i);
            self.events.push(Event::TelegraphEnd { id: lure.id });
            let spells = &mut self.heroes[hero].spell;
            if lure.kind == Mechanic::Spores {
                spells.spore = SPORE_SECS;
            } else {
                spells.bomb = BOMB_FUSE;
            }
            self.events.push(Event::EnemyCast {
                enemy: lure.from,
                visual: SPORE_VISUAL,
                targets: vec![Target::Hero(self.heroes[hero].player)],
                dest: None,
            });
        }
        for h in 0..self.heroes.len() {
            let spells = &mut self.heroes[h].spell;
            spells.spore = (spells.spore - DT).max(0.0);
            if spells.bomb <= 0.0 {
                continue;
            }
            spells.bomb -= DT;
            if spells.bomb <= 0.0 {
                spells.bomb = 0.0;
                self.bomb_blast(h);
            }
        }
    }

    /// A carried bomb goes off around hero `h`: a share of every enemy's health within reach (a
    /// boss's smaller share of its larger health), and a little of the carrier's own, never its
    /// last point.
    fn bomb_blast(&mut self, h: usize) {
        let (player, at) = (self.heroes[h].player, self.heroes[h].pos);
        let mut hit = Vec::new();
        for e in self
            .enemies
            .iter_mut()
            .filter(|e| e.pos.distance(at) <= BOMB_REACH + e.radius)
        {
            let share = if e.boss || e.elite {
                BOMB_BOSS_SHARE
            } else {
                BOMB_SHARE
            };
            let amount = e.max_hp * share;
            e.hp -= amount;
            e.last_hit = Some(player);
            hit.push((e.id, amount));
        }
        for (id, amount) in hit {
            self.events.push(Event::Damage {
                target: Target::Enemy(id),
                amount,
                crit: false,
            });
        }
        let hero = &mut self.heroes[h];
        let cost = (hero.max_hp * BOMB_SELF).min(hero.hp - 1.0).max(0.0);
        hero.hp -= cost;
        if let Some(boss) = self.enemies.iter().find(|e| e.boss).map(|e| e.id) {
            self.events.push(Event::EnemyCast {
                enemy: boss,
                visual: BOMB_VISUAL,
                targets: Vec::new(),
                dest: Some(at),
            });
        }
    }

    /// Where the hazards of `kind` lie, and their shapes.
    pub(super) fn hazard_shapes(&self, kind: Mechanic) -> Vec<(V2, Shape)> {
        self.hazards
            .iter()
            .filter(|h| h.kind == kind)
            .map(|h| (h.pos, h.shape))
            .collect()
    }

    /// Hero `player` thrown or pulled to `to`, within the arena: its own client moves it there.
    fn shove(&mut self, player: PlayerId, to: V2) {
        let center = self.center();
        let leash = self.zone().leash;
        let off = to - center;
        let to = if off.length() > leash {
            center + off.normalize_or_zero() * leash
        } else {
            to
        };
        if let Some(hero) = self.heroes.iter_mut().find(|x| x.player == player) {
            hero.pos = to;
            self.events.push(Event::Shoved { hero: player, to });
        }
    }

    /// A charging or leaping enemy's move this step: true while it is under way.
    pub(super) fn dash_step(e: &mut Enemy) -> bool {
        let Some((to, speed)) = e.act.dash else {
            return false;
        };
        let d = to - e.pos;
        let len = d.length();
        let step = speed * DT;
        e.moving = true;
        if len <= step {
            e.pos = to;
            e.act.dash = None;
        } else {
            e.facing = d.normalize_or_zero();
            e.pos += e.facing * step;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentSources;

    fn shipped() -> Arc<Content> {
        Arc::new(
            Content::from_ron(ContentSources {
                classes: include_str!("../../../../content/classes.ron"),
                abilities: include_str!("../../../../content/abilities.ron"),
                enemies: include_str!("../../../../content/enemies.ron"),
                zones: include_str!("../../../../content/zones.ron"),
            })
            .expect("shipped content is valid"),
        )
    }

    /// A mage with no spells alone in an emptied arena, and an elite (or boss) `key` `dx` yards
    /// away.
    fn duel(key: &str, dx: f32, boss: bool) -> (World, u32) {
        let mut w = World::new(shipped(), "elwynn", 5).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.heroes[0].abilities.clear();
        w.enemies.clear();
        w.interactables.clear();
        w.camps.clear();
        w.pickups.clear();
        w.spawn_timer = f32::MAX;
        w.next_boss = f32::MAX;
        w.heroes[0].max_hp = 1e6;
        w.heroes[0].hp = 1e6;
        let def = w.content.enemies.iter().position(|d| d.key == key).unwrap();
        let at = w.heroes[0].pos + V2::new(dx, 0.0);
        let id = w.spawn_enemy(def, at, w.heroes[0].pos, !boss);
        let e = w.enemies.last_mut().unwrap();
        e.boss = boss;
        // It stands its ground, so a mechanic's range holds until it is ready.
        e.speed = 0.0;
        (w, id)
    }

    fn run(w: &mut World, secs: f32) -> Vec<Event> {
        let mut seen = Vec::new();
        for _ in 0..(secs / DT).round() as usize {
            w.step();
            seen.extend(w.drain_events());
        }
        seen
    }

    fn telegraphs(events: &[Event], kind: Mechanic) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, Event::Telegraph { kind: k, .. } if *k == kind))
            .count()
    }

    #[test]
    fn a_boar_elite_charges_along_a_marked_line() {
        let (mut w, id) = duel("stonetusk_boar", 12.0, false);
        let events = run(&mut w, 4.6);
        assert_eq!(telegraphs(&events, Mechanic::Charge), 1);
        assert!(
            w.heroes[0].hp < w.heroes[0].max_hp,
            "standing in the line, the hero is hit"
        );
        assert!(
            w.enemy(id).unwrap().pos.distance(w.heroes[0].pos) < 6.0,
            "it charged through"
        );
    }

    #[test]
    fn stepping_out_of_the_line_dodges_the_charge() {
        let (mut w, _) = duel("stonetusk_boar", 12.0, false);
        // Until the line is drawn, then a step aside.
        let mut seen = Vec::new();
        while telegraphs(&seen, Mechanic::Charge) == 0 {
            w.step();
            seen.extend(w.drain_events());
            assert!(w.elapsed < 5.0, "no charge came");
        }
        w.heroes[0].pos += V2::new(0.0, 5.0);
        let hp = w.heroes[0].hp;
        run(&mut w, 1.2);
        assert_eq!(w.heroes[0].hp, hp);
    }

    #[test]
    fn a_slam_throws_heroes_out_of_its_circle() {
        let (mut w, _) = duel("kobold_tunneler", 2.0, false);
        let events = run(&mut w, 3.5);
        assert_eq!(telegraphs(&events, Mechanic::Slam), 1);
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Shoved { hero: 0, .. })));
    }

    #[test]
    fn freezing_an_elite_calls_off_its_charge() {
        let (mut w, id) = duel("stonetusk_boar", 12.0, false);
        let mut seen = Vec::new();
        while telegraphs(&seen, Mechanic::Charge) == 0 {
            w.step();
            seen.extend(w.drain_events());
            assert!(w.elapsed < 5.0, "no charge came");
        }
        w.enemies
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .status
            .freeze = 3.0;
        let events = run(&mut w, 1.5);
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::TelegraphEnd { .. })));
        assert_eq!(w.heroes[0].hp, w.heroes[0].max_hp);
    }

    #[test]
    fn ordinary_enemies_have_no_mechanics() {
        let (mut w, _) = duel("stonetusk_boar", 12.0, false);
        w.enemies[0].elite = false;
        let events = run(&mut w, 5.0);
        assert_eq!(telegraphs(&events, Mechanic::Charge), 0);
    }

    #[test]
    fn hogger_charges_and_slams() {
        let (mut w, _) = duel("hogger", 15.0, true);
        let events = run(&mut w, 20.0);
        assert!(telegraphs(&events, Mechanic::Charge) + telegraphs(&events, Mechanic::Slam) > 0);
    }

    #[test]
    fn a_boss_death_grip_pulls_the_hero_to_it() {
        let (mut w, id) = duel("fenros", 30.0, true);
        let events = run(&mut w, 14.0);
        assert!(telegraphs(&events, Mechanic::DeathGrip) >= 1);
        let boss = w.enemy(id).unwrap().pos;
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Shoved { to, .. } if to.distance(boss) < 12.0)));
    }

    #[test]
    fn patches_leave_gaps_to_walk_through() {
        let (mut w, _) = duel("sergeant_brashclaw", 30.0, true);
        w.enemies[0].speed = 0.0;
        let events = run(&mut w, 8.0);
        let spots: Vec<V2> = events
            .iter()
            .filter_map(|e| match e {
                Event::Telegraph {
                    kind: Mechanic::Patches,
                    pos,
                    ..
                } => Some(*pos),
                _ => None,
            })
            .collect();
        assert!(spots.len() >= 3);
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                assert!(a.distance(*b) > PATCH_RADIUS * 2.0);
            }
        }
    }

    /// A lure of `kind` from the boss `id`, laid at `at`.
    fn lure(w: &mut World, kind: Mechanic, id: u32, at: V2) {
        let radius = match kind {
            Mechanic::Spores => SPORE_RADIUS,
            Mechanic::Bomb => BOMB_RADIUS,
            _ => CIRCLE_RADIUS,
        };
        w.mark(kind, at, Shape::Circle { radius }, 10.0, id, 0.0);
    }

    #[test]
    fn walking_into_a_spore_gives_crit() {
        let (mut w, id) = duel("hogger", 30.0, true);
        let at = w.heroes[0].pos + V2::new(0.0, 6.0);
        lure(&mut w, Mechanic::Spores, id, at);
        run(&mut w, 0.2);
        assert_eq!(w.heroes[0].spell.spore, 0.0, "not taken from afar");
        w.heroes[0].pos = at;
        run(&mut w, 0.2);
        assert!(w.heroes[0].spell.spore > 0.0);
        assert!(w.hazards.is_empty(), "the spore is taken");
    }

    #[test]
    fn a_carried_bomb_blasts_the_horde_and_spares_the_carrier() {
        let (mut w, id) = duel("hogger", 5.0, true);
        let near = w.heroes[0].pos + V2::new(-3.0, 0.0);
        let def = w.zone().enemy_ix[0];
        let grunt = w.spawn_enemy(def, near, near, false);
        // No mechanics of its own: only the bomb acts here.
        w.enemies.iter_mut().for_each(|e| {
            e.speed = 0.0;
            e.elite = false;
            e.boss = false;
        });
        w.heroes[0].hp = 5.0;
        let at = w.heroes[0].pos;
        lure(&mut w, Mechanic::Bomb, id, at);
        run(&mut w, 0.2);
        assert!(w.heroes[0].spell.bomb > 0.0, "picked up");
        let boss_hp = w.enemy(id).unwrap().hp;
        run(&mut w, BOMB_FUSE + 0.2);
        assert!(
            w.enemy(id).is_none_or(|e| e.hp < boss_hp),
            "the big one is hit"
        );
        assert!(
            w.enemy(grunt).is_none_or(|e| e.hp < e.max_hp),
            "the horde is hit"
        );
        assert!(
            w.heroes[0].alive && w.heroes[0].hp >= 1.0,
            "never the carrier's last point"
        );
    }

    #[test]
    fn standing_in_the_circle_hurts_the_boss() {
        let (mut w, id) = duel("fenros", 30.0, true);
        let at = w.heroes[0].pos;
        lure(&mut w, Mechanic::Circle, id, at);
        let before = w.enemy(id).unwrap().hp;
        run(&mut w, 2.0);
        let bitten = before - w.enemy(id).unwrap().hp;
        let max = w.enemy(id).unwrap().max_hp;
        assert!(bitten > max * CIRCLE_SHARE, "{bitten}");
    }

    #[test]
    fn shapes_know_who_stands_in_them() {
        let line = Shape::Line {
            to: V2::new(10.0, 0.0),
            width: 2.0,
        };
        assert!(line.contains(V2::ZERO, V2::new(5.0, 0.8), 0.0));
        assert!(!line.contains(V2::ZERO, V2::new(5.0, 2.0), 0.0));
        assert!(!line.contains(V2::ZERO, V2::new(12.0, 0.0), 0.0));
        let circle = Shape::Circle { radius: 3.0 };
        assert!(circle.contains(V2::ZERO, V2::new(2.0, 2.0), 0.0));
    }
}
