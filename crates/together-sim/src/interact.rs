//! Interactables: the shrines, altars and chests that stand about the arena.
//!
//! A hero uses one by standing in its circle until the circle fills ([`Interactable::charge`]);
//! walking away lets it drain again. What it gives goes to every living hero standing in it when
//! it fills, each with their own choice of cards, except a chest, which only its opener pays for
//! and opens. Most are used once a run; a Titan Shrine fills again after a while.
//!
//! Where they stand is rolled per run from the zone's [`Placement`]s ([`scatter`]), on the host,
//! and travels to co-op clients in every [`crate::Snapshot`].

use serde::{Deserialize, Serialize};

use crate::content::ZoneDef;
use crate::math::V2;
use crate::rng::Rng;

/// What an interactable is, and so what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InteractableKind {
    /// A charge shrine: three tomes of rolled rarity. Fills again after a while.
    TitanShrine,
    /// A rare golden charge shrine: three legendary tomes.
    GoldenShrine,
    /// Three pacts, each a strong boon paired with a bane.
    CursedAltar,
    /// Gold now, a harder horde for the rest of the run.
    GreedShrine,
    /// Costs gold, rising with every chest its opener has opened; a pick of loot.
    Chest,
    /// A free chest, left by a won challenge.
    Cache,
    /// Raises a pack of elites around it; killing them all leaves a [`InteractableKind::Cache`].
    ChallengeIdol,
    /// Draws in all the experience on the ground.
    Moonwell,
}

impl InteractableKind {
    pub const ALL: [InteractableKind; 8] = [
        InteractableKind::TitanShrine,
        InteractableKind::GoldenShrine,
        InteractableKind::CursedAltar,
        InteractableKind::GreedShrine,
        InteractableKind::Chest,
        InteractableKind::Cache,
        InteractableKind::ChallengeIdol,
        InteractableKind::Moonwell,
    ];

    pub fn name(self) -> &'static str {
        match self {
            InteractableKind::TitanShrine => "Titan Shrine",
            InteractableKind::GoldenShrine => "Golden Titan Shrine",
            InteractableKind::CursedAltar => "Cursed Altar",
            InteractableKind::GreedShrine => "Shrine of Greed",
            InteractableKind::Chest => "Treasure Chest",
            InteractableKind::Cache => "Champion's Cache",
            InteractableKind::ChallengeIdol => "Challenge Idol",
            InteractableKind::Moonwell => "Moonwell",
        }
    }

    /// One line on what it does, for the prompt a hero near it sees.
    pub fn description(self) -> &'static str {
        match self {
            InteractableKind::TitanShrine => "Choose one of three tomes",
            InteractableKind::GoldenShrine => "Choose one of three legendary tomes",
            InteractableKind::CursedAltar => "Strike a pact: a great boon, and a bane",
            InteractableKind::GreedShrine => "Gold now; the horde grows stronger for good",
            InteractableKind::Chest => "Pay gold for a choice of treasure",
            InteractableKind::Cache => "A free choice of treasure",
            InteractableKind::ChallengeIdol => "Summons elites; slay them all for a cache",
            InteractableKind::Moonwell => "Draws in all experience on the ground",
        }
    }

    /// How far from its centre a hero may stand to use it, in yards.
    pub fn radius(self) -> f32 {
        match self {
            InteractableKind::Chest | InteractableKind::Cache => 2.5,
            _ => 3.0,
        }
    }

    /// Seconds of standing in it to fill the circle.
    pub fn charge_secs(self) -> f32 {
        match self {
            InteractableKind::TitanShrine | InteractableKind::GoldenShrine => 3.0,
            InteractableKind::CursedAltar
            | InteractableKind::GreedShrine
            | InteractableKind::ChallengeIdol => 1.5,
            InteractableKind::Chest | InteractableKind::Cache | InteractableKind::Moonwell => 1.0,
        }
    }

    /// Seconds before it can be used again, or `None` if it is used once.
    pub fn recharge(self) -> Option<f32> {
        match self {
            InteractableKind::TitanShrine => Some(90.0),
            _ => None,
        }
    }
}

/// Where an interactable is in its life.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InteractState {
    /// Waiting for a hero to stand in it.
    Ready,
    /// Used, and ready again in this many seconds.
    Recharging(f32),
    /// A challenge under way: the elites it raised, by enemy id, until every one is dead.
    Challenge(Vec<u32>),
    /// Used up for the run.
    Spent,
}

/// A shrine, altar or chest in the arena.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interactable {
    pub id: u32,
    pub kind: InteractableKind,
    pub pos: V2,
    /// How full the circle is, 0 to 1.
    pub charge: f32,
    pub state: InteractState,
}

impl Interactable {
    pub fn ready(&self) -> bool {
        self.state == InteractState::Ready
    }
}

/// The gold a hero's next chest costs, having opened `opened` already: 20, 40, 70, 110, 160…
pub fn chest_price(opened: u32) -> u32 {
    20 + 15 * opened + 5 * opened * opened
}

/// How many of one kind a zone scatters about its arena each run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Placement {
    pub kind: InteractableKind,
    pub count: u32,
}

/// Tries per interactable at finding a spot clear of the others.
const SCATTER_TRIES: u32 = 40;

/// Spots for every [`Placement`] of `zone`: spread over the arena between a ring kept clear around
/// the centre (where the heroes start) and the leash's edge, each as far as it can be from the
/// rest. Nothing lands within reach of the start.
pub(crate) fn scatter(zone: &ZoneDef, rng: &mut Rng) -> Vec<(InteractableKind, V2)> {
    let center = V2::new(zone.center[0], zone.center[1]);
    let inner = (zone.leash * 0.25).clamp(6.0, 20.0);
    let outer = (zone.leash - 5.0).max(inner + 1.0);
    let gap = (zone.leash * 0.2).clamp(5.0, 14.0);
    let mut placed: Vec<(InteractableKind, V2)> = Vec::new();
    for p in &zone.interactables {
        for _ in 0..p.count {
            let mut best = (center, f32::MIN);
            for _ in 0..SCATTER_TRIES {
                // Even over the ring's area, not bunched at its middle.
                let r = (rng.range(inner * inner, outer * outer)).sqrt();
                let at = center + V2::from_angle(rng.range(0.0, std::f32::consts::TAU), r);
                let clear = placed
                    .iter()
                    .map(|&(_, q)| q.distance(at))
                    .fold(f32::MAX, f32::min);
                if clear > best.1 {
                    best = (at, clear);
                }
                if clear >= gap {
                    break;
                }
            }
            placed.push((p.kind, best.0));
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scatter_places_every_one_inside_the_arena_clear_of_the_start() {
        let content = crate::content::tests::sample();
        let zone = &content.zones[0];
        let want: u32 = zone.interactables.iter().map(|p| p.count).sum();
        assert!(want > 0, "the sample zone places some");
        let spots = scatter(zone, &mut Rng::new(5));
        assert_eq!(spots.len() as u32, want);
        let center = V2::new(zone.center[0], zone.center[1]);
        for &(_, at) in &spots {
            let off = at.distance(center);
            assert!(off >= 20.0 - 1e-3 && off <= zone.leash - 5.0 + 1e-3, "{off}");
        }
        // A wide arena has room to keep them apart.
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                assert!(a.1.distance(b.1) >= 14.0, "{a:?} and {b:?} crowd");
            }
        }
    }

    #[test]
    fn chests_cost_more_each_time() {
        assert_eq!(chest_price(0), 20);
        assert!((0..10).all(|n| chest_price(n + 1) > chest_price(n)));
    }

    #[test]
    fn only_the_titan_shrine_fills_again() {
        for kind in InteractableKind::ALL {
            assert_eq!(
                kind.recharge().is_some(),
                kind == InteractableKind::TitanShrine
            );
        }
    }
}
