//! Rivals out tonight (spec §7.4, and §8.1's "every raid a little different"): the raid's roll
//! says which parties come, when, by which entry and how bold each is; a party comes once no
//! hero of the party who is up stands within `arrive_unseen` yards of its entry, so nobody sees
//! it appear, as the night's newcomers come (sub-project 6).

use serde::{Deserialize, Serialize};

use super::Side;
use crate::combat::Dice;
use crate::raid::seed_for;
use crate::{Content, World, DT, V2};

/// The stream of the raid's seed the rivals' dice come from, so they never follow the loot's or
/// the night's. The seed is hashed ([`seed_for`]), so raids of twin seeds bring different rivals.
const RIVAL_STREAM: u64 = 0x5249_5641_4C00_0007;

/// Seconds between two tries at bringing a waiting party in.
const TRY_EVERY: f32 = 1.0;

/// One rival party out in a raid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RivalOut {
    /// Index into the rivals' parties.
    pub party: u8,
    /// Index into the rivals' entries.
    pub entry: u8,
    /// Seconds into the raid it comes, at the earliest.
    pub at: f32,
    /// Its aggression this raid (spec §7.4), within its kind's.
    pub aggression: f32,
    /// Its dice.
    pub seed: u64,
}

/// The parties out for a raid of `seed`: each comes up at its odds, between `parties_out` of
/// them (the likeliest kept when trimming, the next likeliest added when short); each has an
/// entry, a time within `arrive`, an aggression within its kind's, and dice of its own. None
/// when the content has no parties.
pub(crate) fn pick_rivals(content: &Content, seed: u64) -> Vec<RivalOut> {
    let rc = &content.rivals;
    let (lo, hi) = content.rules.rivals.parties_out;
    let (early, late) = content.rules.rivals.arrive;
    if rc.parties.is_empty() || rc.entries.is_empty() {
        return Vec::new();
    }
    let mut dice = Dice::new(seed_for(seed, RIVAL_STREAM));
    // A key under 1 comes up at its odds; the lowest keys are the likeliest.
    let mut keyed: Vec<(f32, usize)> = rc
        .parties
        .iter()
        .enumerate()
        .filter(|(_, p)| p.odds > 0.0)
        .map(|(i, p)| (dice.unit() / p.odds, i))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let rolled = keyed.iter().filter(|(k, _)| *k < 1.0).count();
    let fewest = usize::from(lo).min(keyed.len());
    let take = rolled.clamp(fewest, usize::from(hi).max(fewest));
    let mut out: Vec<usize> = keyed.iter().take(take).map(|&(_, i)| i).collect();
    out.sort_unstable();
    let entries = rc.entries.len();
    out.iter()
        .enumerate()
        .map(|(n, &i)| {
            let p = &rc.parties[i];
            RivalOut {
                party: i as u8,
                entry: ((dice.unit() * entries as f32) as usize).min(entries - 1) as u8,
                at: dice.between(early, late),
                aggression: dice.between(p.aggression.0, p.aggression.1),
                seed: seed_for(seed ^ RIVAL_STREAM, n as u64 + 1),
            }
        })
        .collect()
}

impl World {
    /// The roll's parties come in: each once its time has come and, tried every second, once no
    /// hero of the party who is up stands within `arrive_unseen` yards of its entry. Only once a
    /// map is attached (parties walk it).
    pub(crate) fn step_rival_arrivals(&mut self) {
        if self.raid.roll.rivals.is_empty() || self.map().is_none() {
            return;
        }
        let count = self.raid.roll.rivals.len();
        if self.rivals.arrived.len() < count {
            self.rivals.arrived.resize(count, false);
        }
        self.rivals.try_in -= DT;
        if self.rivals.try_in > 0.0 {
            return;
        }
        self.rivals.try_in = TRY_EVERY;
        let content = self.content().clone();
        let unseen = content.rules.night.arrive_unseen;
        for n in 0..count {
            let o = self.raid.roll.rivals[n];
            if self.rivals.arrived[n] || self.elapsed < o.at {
                continue;
            }
            let at: V2 = content.rivals.entries[usize::from(o.entry)];
            let near = self
                .heroes
                .iter()
                .any(|h| h.side == Side::Party && h.fight.up() && h.pos.distance(at) <= unseen);
            if near {
                continue;
            }
            self.rivals.arrived[n] = true;
            self.add_party(usize::from(o.party), n as u8, o.aggression, at, o.seed);
        }
    }
}
