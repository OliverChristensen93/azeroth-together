//! Rivals' senses (spec §7.4: "the same sight, noise and awareness rules as heroes"), both
//! ways. Enemies see and hear rivals already: rivals are heroes to them. This file is the other
//! way and the heroes' view: each rival keeps a [`Mind`] of the party's heroes, fed by the
//! senses' own [`sense`] (the notice range by the hero's visibility and the rival's facing, line
//! of sight within the fog, noises heard through walls at half their radius); a rival that
//! becomes alerted shouts to its party; a hero's blow alerts it, and lands as an ambush while it
//! is unaware; a Vanish loses it; a hero it is alerted to is in combat. A hero sees a rival in the
//! fog and in line of sight, a stealthed one only within the notice range its visibility gives
//! (spec §5.1), as an enemy sees a stealthed hero; the rivals' marks ("?" and "!") are each
//! player's, as enemies' are.

use super::{RivalMark, RivalView, Side};
use crate::senses::{notice_range, sense, Mind, Sensed};
use crate::{Hero, PlayerId, UnitRef, SenseRules, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

impl World {
    /// Every rival up and in the fight senses the party's heroes and thinks (spec §5.3); one that
    /// becomes alerted shouts to its party within `shout` yards. Before the enemies take the
    /// step's noises (`step_enemies` empties them), so what rivals hear is what was made this
    /// step. A rival held out of the fight neither looks nor thinks.
    pub(crate) fn perceive_rivals(&mut self) {
        // The loudest noise a hero of the party made lately (the night's hunt memory).
        let memory = self.content().rules.night.hunt.memory;
        let now = self.elapsed;
        if self.rivals.rumour.is_some_and(|(_, _, t)| now - t > memory) {
            self.rivals.rumour = None;
        }
        let loudest = self
            .senses
            .noises
            .iter()
            .filter(|n| self.side_of(n.by) == Side::Party && self.hero(n.by).is_some())
            .max_by(|a, b| a.radius.total_cmp(&b.radius))
            .map(|n| (n.at, n.radius));
        if let Some((at, radius)) = loudest {
            if self.rivals.rumour.is_none_or(|(_, r, _)| radius >= r) {
                self.rivals.rumour = Some((at, radius, now));
            }
        }
        let Some(map) = self.map().cloned() else {
            return;
        };
        let content = self.content().clone();
        let r = &content.rules.senses;
        let fog = self.fog().end;
        let notice = self.notice_scale();
        let mut shouts: Vec<(u8, V2, PlayerId, V3)> = Vec::new();
        for b in 0..self.rivals.bodies.len() {
            let player = self.rivals.bodies[b].player;
            let Some(me) = self.hero(player) else {
                continue;
            };
            let out = self
                .combat
                .hero(player)
                .is_some_and(|c| c.held.is_some_and(|h| h.kind.out()));
            if !me.fight.up() || out {
                continue;
            }
            let (pos, facing) = (me.pos, me.facing);
            let eye = V3::at(me.pos, me.z + EYE_HEIGHT);
            let party = self.rivals.bodies[b].party;
            let mind = &self.rivals.bodies[b].mind;
            let sensed: Vec<Sensed> = self
                .heroes
                .iter()
                .filter(|h| h.side == Side::Party)
                .map(|h| {
                    sense(
                        pos,
                        facing,
                        mind,
                        eye,
                        h,
                        &self.senses.noises,
                        &map,
                        fog,
                        notice,
                        r,
                    )
                })
                .collect();
            if let Some(s) = self.rivals.bodies[b].mind.think(&sensed, DT, &r.awareness) {
                shouts.push((party, pos, s.player, s.at));
            }
        }
        for (party, from, player, at) in shouts {
            self.party_shout(party, from, player, at);
        }
    }

    /// Every member of rival party `party` up within `shout` yards of `from`, not yet alerted, is
    /// alerted to `player`, last seen at `at`. Shouts do not chain, as enemies' do not.
    fn party_shout(&mut self, party: u8, from: V2, player: PlayerId, at: V3) {
        let content = self.content().clone();
        let r = &content.rules.senses.awareness;
        let near: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.side == Side::Rival(party) && h.fight.up())
            .filter(|h| h.pos.distance(from) <= r.shout)
            .map(|h| h.player)
            .collect();
        for b in &mut self.rivals.bodies {
            if near.contains(&b.player) && !b.mind.is_alerted() {
                b.mind.alert(player, at, r);
            }
        }
    }

    /// Rival `rival` is alerted to `by` where `by` stands now (a blow, a hostile ability), its
    /// party is told within `shout` yards, and the party counts as struck by a hero.
    pub(crate) fn alert_rival(&mut self, rival: PlayerId, by: PlayerId) {
        let Side::Rival(party) = self.side_of(rival) else {
            return;
        };
        if !self.hostile(rival, by) {
            return;
        }
        let (Some(at), Some(from)) = (
            self.hero(by).map(|h| V3::at(h.pos, h.z)),
            self.hero(rival).map(|h| h.pos),
        ) else {
            return;
        };
        let content = self.content().clone();
        if let Some(b) = self.rivals.body_mut(rival) {
            b.mind.alert(by, at, &content.rules.senses.awareness);
        }
        self.party_shout(party, from, by, at);
        for p in &mut self.rivals.parties {
            if p.number == party {
                p.hurt_by = Some(by);
            }
        }
    }

    /// Every rival's mind loses `player` (their Vanish): an alert turns to a search where they
    /// were last seen.
    pub(crate) fn rivals_lose(&mut self, player: PlayerId) {
        let content = self.content().clone();
        for b in &mut self.rivals.bodies {
            b.mind.lose(player, &content.rules.senses.awareness);
        }
    }

    /// Rival `player`'s mind.
    pub(crate) fn rival_mind(&self, player: PlayerId) -> Option<&Mind> {
        self.rivals.body(player).map(|b| &b.mind)
    }

    /// Whether a rival up is alerted to hero `player`: the hero is hunted, so in combat as when
    /// an enemy is (no regeneration, no stepping into stealth with a rival on their heels).
    pub(crate) fn rival_hunts(&self, player: PlayerId) -> bool {
        self.rivals
            .bodies
            .iter()
            .any(|b| b.mind.alerted_to(player) && self.hero(b.player).is_some_and(|h| h.fight.up()))
    }

    /// What `player`'s client is told of the rivals: those their hero sees (spec §4.3) and the
    /// marks living rivals show them. On a client's mirror, what its host sent.
    pub fn rival_view(&self, player: PlayerId) -> RivalView {
        if let Some(v) = &self.rivals.mirror {
            return v.clone();
        }
        let r = &self.content().rules.senses.awareness;
        RivalView {
            seen: self.rivals_seen_by(player),
            marks: self
                .rivals
                .bodies
                .iter()
                .filter(|b| self.hero(b.player).is_some_and(|h| h.fight.up()))
                .filter(|b| self.in_reach(player, UnitRef::Hero(b.player)))
                .filter_map(|b| {
                    b.mind.mark(player, r).map(|alert| RivalMark {
                        rival: b.player,
                        alert,
                    })
                })
                .collect(),
        }
    }

    /// The rivals `player`'s hero sees, or a teammate of theirs within the shared-sight reach.
    fn rivals_seen_by(&self, player: PlayerId) -> Vec<PlayerId> {
        let Some(me) = self.hero(player) else {
            return Vec::new();
        };
        let content = self.content();
        let share = content.rules.world.share_sight_within;
        let fog = self.fog().end;
        let watchers: Vec<&Hero> = self
            .heroes
            .iter()
            .filter(|h| {
                h.player == player || (h.side == me.side && h.pos.distance(me.pos) <= share)
            })
            .collect();
        self.heroes
            .iter()
            .filter(|h| h.side.is_rival() && h.side != me.side)
            .filter(|h| {
                watchers
                    .iter()
                    .any(|w| self.sees_rival(w, h, fog, &content.rules.senses))
            })
            .map(|h| h.player)
            .collect()
    }

    /// Whether `w` sees rival `h`: within the fog (a stealthed one within the notice range its
    /// visibility gives, faced), and nothing solid between `w`'s eyes and its chest.
    fn sees_rival(&self, w: &Hero, h: &Hero, fog: f32, r: &SenseRules) -> bool {
        let range = if h.stealthed {
            notice_range(h.visibility, 1.0, fog, r)
        } else {
            fog
        };
        w.pos.distance(h.pos) <= range
            && self.clear_line(
                V3::at(w.pos, w.z + EYE_HEIGHT),
                V3::at(h.pos, h.z + CHEST_HEIGHT),
            )
    }
}
