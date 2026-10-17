//! Enemies held and hurt over time (spec §6.2, §6.3): holds running out (a stun, a sap, a sheep, a
//! root), slows and distractions, bleeds and burns ticking, a sheep wandering where it was
//! turned, what is on each enemy written for the clients ([`EnemyFight`]), and what its senses may
//! do each step ([`Engage`]): look, think and move, stand, or nothing at all; how close it closes
//! on its hero (a caster keeps its distance while it can see them); and how fast.

use super::{Casting, EnemyFight, Engage, Event, Hold, Stop, UnitRef};
use crate::{Aim, Content, ControlKind, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

/// Yards inside its longest spell's range a caster stops.
const KEEP_INSIDE: f32 = 2.0;
/// Yards around where it was turned a sheep wanders.
const SHEEP_ROAM: f32 = 4.0;
/// Within this many yards of its spot a sheep picks another.
const SHEEP_THERE: f32 = 0.3;
/// A move reaching further than this many yards is a caster's: it stands off to use it. (Nearer
/// ones are melee and chase at the content's `chase_reach`.)
const RANGED: f32 = 8.0;

impl World {
    /// Every enemy's holds, slows, distractions, bleeds and burns move on a step; a sheep
    /// wanders; what is on each is written for the clients; and what its senses may do is set.
    pub(crate) fn hold_enemies(&mut self) {
        let content = self.content.clone();
        for ei in 0..self.enemies.len().min(self.combat.enemies.len()) {
            self.tick_holds(ei);
            self.tick_burns(ei);
            self.wander(ei, &content);
            let engage = self.engage(ei, &content);
            if engage.hold == Hold::Out && !self.is_sheep(ei) {
                self.enemies[ei].speed = 0.0;
            }
            self.combat.enemies[ei].engage = engage;
            let fight = self.enemy_fight(ei);
            self.enemies[ei].fight = fight;
        }
    }

    fn is_sheep(&self, ei: usize) -> bool {
        self.combat.enemies[ei]
            .held
            .is_some_and(|h| h.kind == ControlKind::Polymorph)
    }

    /// Holds, slows and distractions run down; a distraction ends once the enemy is after
    /// someone; a hold that takes it out of the fight stops its cast, and the blow it was about to
    /// land on the ground.
    fn tick_holds(&mut self, ei: usize) {
        let alerted = self.senses.alerted(ei);
        let id = self.enemies[ei].id;
        let ec = &mut self.combat.enemies[ei];
        // A time that is not a number never runs out by itself, so it is dropped at once.
        if let Some(h) = &mut ec.held {
            h.left -= DT;
            if h.left <= 1e-4 || !h.left.is_finite() {
                ec.held = None;
                ec.wander = None;
            }
        }
        if let Some(s) = &mut ec.slow {
            s.left -= DT;
            if s.left <= 1e-4 || !s.left.is_finite() {
                ec.slow = None;
            }
        }
        if let Some((_, left)) = &mut ec.distracted {
            *left -= DT;
            if *left <= 1e-4 || !left.is_finite() || alerted {
                ec.distracted = None;
            }
        }
        if ec.held.is_some_and(|h| h.kind.out()) {
            if let Some(c) = ec.cast.take() {
                if let Some(t) = c.telegraph {
                    self.telegraphs.retain(|x| x.id != t);
                }
                self.events.push(Event::CastStop {
                    by: UnitRef::Enemy(id),
                    ability: c.ability as u16,
                    why: Stop::Interrupted,
                });
            }
        }
    }

    /// Bleeds and burns on enemy `ei` tick (never a crit, as in WoW), and run out.
    fn tick_burns(&mut self, ei: usize) {
        if self.enemies[ei].hp <= 0.0 {
            self.combat.enemies[ei].burns.clear();
            return;
        }
        let mut due = Vec::new();
        for b in &mut self.combat.enemies[ei].burns {
            b.next -= DT;
            if b.next <= 1e-4 {
                // A period that is not a positive number would tick every step for ever: such a
                // burn ticks once and ends.
                let every = if b.every.is_finite() && b.every > 0.0 {
                    b.every
                } else {
                    f32::INFINITY
                };
                b.next += every;
                b.left -= every;
                due.push((b.by, b.amount, b.silent));
            }
        }
        self.combat.enemies[ei]
            .burns
            .retain(|b| b.left > 1e-4 && b.left.is_finite());
        for (by, amount, silent) in due {
            self.strike(by, ei, amount, false, silent, false);
        }
    }

    /// A sheep walks to a spot within [`SHEEP_ROAM`] of where it was turned, then another,
    /// never through a wall.
    fn wander(&mut self, ei: usize, content: &Content) {
        if !self.is_sheep(ei) {
            return;
        }
        let Some(map) = self.map().cloned() else {
            return;
        };
        let speed = content.rules.combat.sheep_speed;
        let e = &mut self.enemies[ei];
        // A sleeper turned into a sheep is awake.
        e.asleep = false;
        let ec = &mut self.combat.enemies[ei];
        let (home, mut goal) = ec.wander.unwrap_or((e.pos, e.pos));
        if goal.distance(e.pos) <= SHEEP_THERE {
            let angle = self.combat.dice.unit() * std::f32::consts::TAU;
            let far = self.combat.dice.unit() * SHEEP_ROAM;
            goal = home + V2::from_angle(angle, far);
        }
        let (here, to) = (V3::at(e.pos, e.z), V3::at(goal, e.z));
        if !map.walk_line(here, to) {
            ec.wander = Some((home, e.pos));
            e.speed = 0.0;
            return;
        }
        let mut next = 0;
        crate::enemies::follow(e, &[to], &mut next, speed, &map);
        ec.wander = Some((home, goal));
    }

    /// What enemy `ei`'s senses may do this step.
    pub(super) fn engage(&self, ei: usize, content: &Content) -> Engage {
        let (e, ec) = (&self.enemies[ei], &self.combat.enemies[ei]);
        let hold = if e.hp <= 0.0 {
            Hold::Out
        } else if let Some(h) = ec.held {
            if h.kind.out() {
                Hold::Out
            } else {
                Hold::Still
            }
        } else if ec.cast.is_some() {
            Hold::Still
        } else if let Some((spot, _)) = ec.distracted {
            Hold::Facing(spot)
        } else {
            Hold::Free
        };
        Engage {
            hold,
            reach: self
                .keep_at(ei, content)
                .unwrap_or(content.rules.senses.chase_reach),
            // A slow whose strength is not a number slows nothing; one beyond 0..1 is cut to it.
            speed: 1.0
                - ec.slow.map_or(0.0, |s| {
                    if s.by.is_finite() {
                        s.by.clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                }),
        }
    }

    /// How far a caster stands off the hero it is after: its longest spell's range, a little
    /// inside, while it has a clear line to them; `None` for one that is no caster, or blind to
    /// its hero (it then closes in until it sees them).
    fn keep_at(&self, ei: usize, content: &Content) -> Option<f32> {
        let e = &self.enemies[ei];
        let range = content.enemies[e.kind]
            .kit
            .iter()
            .map(|&(a, _)| &content.abilities[a])
            .filter(|a| a.aim == Aim::Enemy && a.telegraph.is_none() && a.range > RANGED)
            .map(|a| a.range)
            .fold(0.0f32, f32::max);
        if range <= 0.0 {
            return None;
        }
        let (player, _) = self.senses.target_of(ei)?;
        let h = self.hero(player)?;
        // As for a hero's own blows, a clear line at every range.
        let sees = self.clear_line(
            V3::at(e.pos, e.z + EYE_HEIGHT),
            V3::at(h.pos, h.z + CHEST_HEIGHT),
        );
        sees.then_some((range - KEEP_INSIDE).max(content.rules.senses.chase_reach))
    }

    /// What every client sees of enemy `ei`'s fight.
    pub(super) fn enemy_fight(&self, ei: usize) -> EnemyFight {
        let ec = &self.combat.enemies[ei];
        let mut auras: Vec<u32> = Vec::new();
        let spells = ec
            .held
            .map(|h| h.spell)
            .into_iter()
            .chain(ec.slow.map(|s| s.spell))
            .chain(ec.burns.iter().map(|b| b.spell));
        for s in spells {
            if !auras.contains(&s) {
                auras.push(s);
            }
        }
        EnemyFight {
            control: ec.held.map(|h| h.kind),
            slowed: ec.slow.is_some(),
            casting: ec.cast.map(|c| Casting {
                ability: c.ability as u16,
                left: c.left.max(0.0),
                total: c.total,
                channel: false,
                target: Some(UnitRef::Hero(c.player)),
            }),
            auras,
        }
    }
}
