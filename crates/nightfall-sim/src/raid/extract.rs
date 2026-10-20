//! The end of a hero's raid (spec §8.1, §8.2): out by an exit after `extract_for` seconds in its
//! ring, or dead with their body left holding what `death_loss` takes; the raid over once every
//! hero is out or dead, each hero's outcome settled once and handed out once.

use super::{Exiting, Fate, HeroOutcome, Holder, Raid, RaidRoll, Recovery};
use crate::{
    Content, DeathLoss, Event, Inventory, Life, Loudness, PlayerId, Side, Stack, World, DT, V2, V3,
};

/// The exit (an index into the map's exits) whose ring `pos` stands in while it is open now
/// (opened, and not closed by the hunt). Rings are flat: no exit stands over another floor.
pub(crate) fn exit_at(content: &Content, roll: &RaidRoll, now: f32, pos: V2) -> Option<usize> {
    content
        .map
        .exits
        .iter()
        .enumerate()
        .find(|(i, e)| {
            roll.exit_open(*i, now) && pos.distance(e.at) <= e.radius
        })
        .map(|(i, _)| i)
}

impl World {
    /// Every enemy killed since the last tally counts for the hero who killed it, with its kind
    /// and where it fell. Last in a step, so every kill of the step counts;
    /// [`World::drain_events`] tallies what is left and starts the count over, so no kill is
    /// counted twice or missed, whoever drains the events. The count is the world's own, never a
    /// player's event stream (which is cut to their reach).
    pub(crate) fn tally_kills(&mut self) {
        if self.raid.mirror.is_some() {
            return;
        }
        let from = self.raid.counted.min(self.events.len());
        let by: Vec<(PlayerId, u32)> = self.events[from..]
            .iter()
            .filter_map(|e| match e {
                Event::EnemyDied { id, by: Some(p) } => Some((*p, *id)),
                _ => None,
            })
            .collect();
        // A rival slain counts as a kill, but is no enemy of the book of kinds slain.
        let rivals: Vec<PlayerId> = self.events[from..]
            .iter()
            .filter_map(|e| match e {
                Event::RivalDied { by: Some(p), .. } => Some(*p),
                _ => None,
            })
            .collect();
        self.raid.counted = self.events.len();
        for p in rivals {
            self.raid.add_kill(p);
        }
        for (p, id) in by {
            self.raid.add_kill(p);
            if let Some(e) = self.enemies.iter().find(|e| e.id == id) {
                self.raid.add_slain(p, e.kind as u16, e.pos);
            }
        }
    }

    /// Heroes up in an open exit's ring count to `extract_for` (spec §8.1), heard as they step in
    /// and every second after (§5.2); a blow, stepping out or falling starts the count over.
    /// At the end of the count they are out.
    pub(crate) fn step_exits(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.run;
        let now = self.elapsed;
        let mut ticks: Vec<(PlayerId, V3)> = Vec::new();
        let mut out: Vec<(PlayerId, usize)> = Vec::new();
        {
            let Raid { exiting, roll, .. } = &mut self.raid;
            for h in self.heroes.iter_mut() {
                let i = match exiting.iter().position(|e| e.player == h.player) {
                    Some(i) => i,
                    None => {
                        exiting.push(Exiting {
                            player: h.player,
                            t: 0.0,
                            struck: false,
                        });
                        exiting.len() - 1
                    }
                };
                let e = &mut exiting[i];
                let hurt = std::mem::take(&mut e.struck);
                let ring = if h.fight.up() {
                    exit_at(&content, roll, now, h.pos)
                } else {
                    None
                };
                match ring {
                    Some(x) if !hurt => {
                        if e.t == 0.0 || (e.t + DT).floor() > e.t.floor() {
                            ticks.push((h.player, V3::at(h.pos, h.z)));
                        }
                        e.t += DT;
                        if e.t + 1e-4 >= r.extract_for {
                            out.push((h.player, x));
                        }
                    }
                    _ => e.t = 0.0,
                }
                h.carry.extracting = e.t;
            }
        }
        for (p, at) in ticks {
            self.make_noise(p, at, Loudness::Extract);
        }
        for (p, x) in out {
            self.extract(p, x);
        }
    }

    /// `player` is out by exit `exit`: everything they carry is theirs to keep, and they leave
    /// the world.
    pub(super) fn extract(&mut self, player: PlayerId, exit: usize) {
        // A rival is out as a hero is, but the raid is not its: no outcome (spec §7.4).
        if self.side_of(player).is_rival() {
            if self.hero(player).is_some_and(|h| h.fight.up()) {
                self.rival_leaves(player, exit);
            }
            return;
        }
        if self.raid.over || !self.hero(player).is_some_and(|h| h.fight.up()) {
            return;
        }
        let content = self.content().clone();
        let mut carried = self
            .raid
            .take_pack(player)
            .unwrap_or_else(|| Inventory::new(&content));
        carried.clear_found();
        let fate = Fate::Extracted {
            exit: content
                .map
                .exits
                .get(exit)
                .map_or_else(String::new, |e| e.name.clone()),
        };
        let o = self.outcome(player, fate, self.elapsed, carried, Vec::new(), None);
        self.raid.outcomes.push(o);
        self.remove_hero(player);
    }

    /// Every hero dead since the last step leaves their body where they fell (spec §8.2),
    /// holding what `death_loss` takes; friends can search it this raid. A rival's body holds
    /// everything it had (spec §7.4), and its death is told.
    pub(crate) fn step_deaths(&mut self) {
        if self.raid.mirror.is_some() {
            return;
        }
        let content = self.content().clone();
        let dead: Vec<(PlayerId, V2, f32, bool)> = self
            .heroes
            .iter()
            .filter(|h| {
                h.fight.life == Life::Dead
                    && !self.raid.fallen.iter().any(|(p, _, _)| *p == h.player)
            })
            .map(|h| (h.player, h.pos, h.z, h.side.is_rival()))
            .collect();
        for (player, at, z, rival) in dead {
            let mode = if rival {
                DeathLoss::Everything
            } else {
                content.rules.run.death_loss
            };
            let mut death = self
                .raid
                .pack_mut(player, &content)
                .split_for_death(mode, &content);
            // A rival's kit is the night's, not the looter's own: on its body it is found, so a
            // `found_only` death takes it from whoever carries it out.
            if rival {
                for s in &mut death.lost {
                    s.found = true;
                }
            }
            // What was brought stays the hero's even when it no longer fits their backpack: it
            // goes to their outcome, never onto the body.
            if !death.spill.is_empty() {
                self.raid.spilled.push((player, death.spill));
            }
            let body = if death.lost.is_empty() {
                0
            } else {
                // A rival's whole kit fits its body, whatever a hero's holds.
                let slots = usize::from(content.rules.run.container_slots).max(death.lost.len());
                self.raid.add_container(
                    Holder::Hero(player),
                    at,
                    Some(z),
                    death.lost,
                    slots,
                    false,
                    None,
                )
            };
            self.raid.fallen.push((player, body, self.elapsed));
            self.raid.close(player);
            if let Some(hi) = self.hero_slot(player) {
                self.heroes[hi].carry.lantern = None;
            }
            self.refresh_carry(player);
            if rival {
                let by = self.rivals.struck_by(player);
                self.events.push(Event::RivalDied { player, by });
            }
        }
    }

    /// With every hero of the party out or dead the raid is over (spec §8.1), and each dead
    /// hero's outcome is settled now: what is left on their body by then is what a recovery
    /// holds. Rivals do not count (spec §7.4): the raid is the party's, so it ends with them
    /// whatever the rivals do, and settles no outcome for a rival. A client's mirror has no raid
    /// to end: its host tells it.
    pub(crate) fn step_over(&mut self) {
        if self.raid.mirror.is_some() {
            return;
        }
        if self.heroes.iter().any(|h| h.side == Side::Party) {
            self.raid.had_heroes = true;
        }
        if self.raid.over || !self.raid.had_heroes {
            return;
        }
        if !self
            .heroes
            .iter()
            .filter(|h| h.side == Side::Party)
            .all(|h| h.fight.life == Life::Dead)
        {
            return;
        }
        // Every hero who has died is on the books before anyone is settled.
        self.step_deaths();
        self.raid.over = true;
        let dead: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.side == Side::Party)
            .map(|h| h.player)
            .collect();
        for p in dead {
            self.settle_dead(p);
        }
    }

    /// Dead `player`'s outcome: what death left them, and under `everything_one_recovery` their
    /// body's remaining things as the recovery their next raid holds.
    fn settle_dead(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let mut carried = self
            .raid
            .take_pack(player)
            .unwrap_or_else(|| Inventory::new(&content));
        carried.clear_found();
        let overflow = self.raid.take_spilled(player);
        // When they fell is when their raid ended, not when the rest of it did.
        let (body, fell) = self
            .raid
            .fallen
            .iter()
            .find(|(p, _, _)| *p == player)
            .map_or((0, self.elapsed), |(_, id, t)| (*id, *t));
        let recovery = if content.rules.run.death_loss == DeathLoss::EverythingOneRecovery {
            self.raid
                .containers
                .iter()
                .find(|c| c.id == body)
                .and_then(|c| {
                    let items: Vec<Stack> = c
                        .stacks
                        .iter()
                        .flatten()
                        .map(|s| Stack { found: false, ..*s })
                        .collect();
                    (!items.is_empty()).then(|| Recovery {
                        map: content.map.key.clone(),
                        at: V3::at(c.at, c.z.unwrap_or(0.0)),
                        items,
                    })
                })
        } else {
            None
        };
        let o = self.outcome(player, Fate::Died, fell, carried, overflow, recovery);
        self.raid.outcomes.push(o);
    }

    /// `player`'s outcome, final as `time` seconds into the raid.
    fn outcome(
        &mut self,
        player: PlayerId,
        fate: Fate,
        time: f32,
        carried: Inventory,
        overflow: Vec<Stack>,
        recovery: Option<Recovery>,
    ) -> HeroOutcome {
        // A kill made earlier in this very step counts, though the step has not tallied yet.
        self.tally_kills();
        let name = self
            .hero(player)
            .map(|h| h.name.clone())
            .unwrap_or_default();
        let mut recovered = self.raid.take_recovered(player);
        for s in &mut recovered {
            s.found = false;
        }
        HeroOutcome {
            player,
            name,
            fate,
            time,
            kills: self.raid.kills_of(player),
            carried,
            overflow: overflow
                .into_iter()
                .map(|s| Stack { found: false, ..s })
                .collect(),
            recovered,
            recovery,
            slain: self.raid.slain_of(player),
        }
    }

    /// The outcomes settled since the last call: each handed out once, for the host to give to
    /// its player.
    pub fn take_outcomes(&mut self) -> Vec<HeroOutcome> {
        let from = self.raid.taken.min(self.raid.outcomes.len());
        let new = self.raid.outcomes[from..].to_vec();
        self.raid.taken = self.raid.outcomes.len();
        new
    }

    /// Every outcome so far, for the end screen.
    pub fn outcomes(&self) -> &[HeroOutcome] {
        &self.raid.outcomes
    }

    /// Every hero is out or dead, or the raid was abandoned.
    pub fn is_over(&self) -> bool {
        self.raid.over
    }

    /// The raid ends now (its host abandoned it): every hero still in it is missing, losing what
    /// a death would and leaving no body; the dead are settled as dead.
    pub fn abandon(&mut self) {
        if self.raid.over {
            return;
        }
        let content = self.content().clone();
        self.raid.over = true;
        self.raid.had_heroes = true;
        // A hero who died since the last step has left their body before anyone is settled.
        self.step_deaths();
        // Kills since the last step still count.
        self.tally_kills();
        let heroes: Vec<(PlayerId, bool)> = self
            .heroes
            .iter()
            .filter(|h| h.side == Side::Party)
            .map(|h| (h.player, h.fight.life == Life::Dead))
            .collect();
        for (p, dead) in heroes {
            if dead {
                self.settle_dead(p);
                continue;
            }
            let mut carried = self
                .raid
                .take_pack(p)
                .unwrap_or_else(|| Inventory::new(&content));
            // The missing lose what a death would; what was brought stays theirs, as in a death.
            let death = carried.split_for_death(content.rules.run.death_loss, &content);
            carried.clear_found();
            let overflow = death.spill;
            let o = self.outcome(p, Fate::Missing, self.elapsed, carried, overflow, None);
            self.raid.outcomes.push(o);
        }
    }
}
