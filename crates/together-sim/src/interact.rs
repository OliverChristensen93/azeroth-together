//! Interactables: the shrines, altars, chests, banners and pots that stand about the arena.
//!
//! A hero uses one by standing in its circle until the circle fills ([`Interactable::charge`]);
//! walking away lets it drain again. A pot breaks at a touch. What a shrine gives goes to every
//! living hero standing in it when it fills, each with their own choice of cards, except a chest,
//! which only its opener pays for and opens. Each is used once, then goes.
//!
//! Only a few stand at once: the zone says how many ([`ZoneDef::shrines_at_once`]) and of which
//! kinds ([`Placement`]). The run starts with them spread over the arena ([`scatter`]), and new
//! ones appear out of sight of the heroes as old ones are used. All of it is rolled on the host,
//! and travels to co-op clients in every [`crate::Snapshot`].

use serde::{Deserialize, Serialize};

use crate::content::ZoneDef;
use crate::math::V2;
use crate::rng::Rng;
use crate::world::PlayerId;

/// What an interactable is, and so what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InteractableKind {
    /// A charge shrine: three tomes of rolled rarity.
    TitanShrine,
    /// A rare golden charge shrine: three legendary tomes.
    GoldenShrine,
    /// Three pacts, each a strong boon paired with a bane.
    CursedAltar,
    /// Costs gold, rising with every chest its opener has opened; a pick of items.
    Chest,
    /// A free chest, left by a won challenge, a carried banner or a dead boss.
    Cache,
    /// Raises a pack of elites around it; killing them all leaves a [`InteractableKind::Cache`].
    ChallengeIdol,
    /// Taken up, it must be carried to a marked spot in time; there a cache waits.
    WarBanner,
    /// Breaks at a touch: coins, experience, a health shard or a power-up. Scattered over the
    /// arena, with a new one now and then as they break.
    Pot,
}

impl InteractableKind {
    pub const ALL: [InteractableKind; 8] = [
        InteractableKind::TitanShrine,
        InteractableKind::GoldenShrine,
        InteractableKind::CursedAltar,
        InteractableKind::Chest,
        InteractableKind::Cache,
        InteractableKind::ChallengeIdol,
        InteractableKind::WarBanner,
        InteractableKind::Pot,
    ];

    pub fn name(self) -> &'static str {
        match self {
            InteractableKind::TitanShrine => "Titan Shrine",
            InteractableKind::GoldenShrine => "Golden Titan Shrine",
            InteractableKind::CursedAltar => "Cursed Altar",
            InteractableKind::Chest => "Treasure Chest",
            InteractableKind::Cache => "Champion's Cache",
            InteractableKind::ChallengeIdol => "Challenge Idol",
            InteractableKind::WarBanner => "War Banner",
            InteractableKind::Pot => "Titan Urn",
        }
    }

    /// One line on what it does, for the prompt a hero near it sees.
    pub fn description(self) -> &'static str {
        match self {
            InteractableKind::TitanShrine => "Choose one of three tomes",
            InteractableKind::GoldenShrine => "Choose one of three legendary tomes",
            InteractableKind::CursedAltar => "Strike a pact: a great boon, and a bane",
            InteractableKind::Chest => "Pay gold for a choice of treasure",
            InteractableKind::Cache => "A free choice of treasure",
            InteractableKind::ChallengeIdol => "Summons elites; slay them all for a cache",
            InteractableKind::WarBanner => "Carry it to the marked spot in time for a cache",
            InteractableKind::Pot => "Break it for what is inside",
        }
    }

    /// How far from its centre a hero may stand to use it, in yards.
    pub fn radius(self) -> f32 {
        match self {
            InteractableKind::Chest | InteractableKind::Cache => 2.5,
            InteractableKind::Pot => 1.0,
            _ => 3.0,
        }
    }

    /// Seconds of standing in it to fill the circle; a pot breaks at once.
    pub fn charge_secs(self) -> f32 {
        match self {
            InteractableKind::TitanShrine | InteractableKind::GoldenShrine => 3.0,
            InteractableKind::CursedAltar
            | InteractableKind::ChallengeIdol
            | InteractableKind::WarBanner => 1.5,
            InteractableKind::Chest | InteractableKind::Cache => 1.0,
            InteractableKind::Pot => 0.0,
        }
    }
}

/// Where an interactable is in its life.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InteractState {
    /// Waiting for a hero to stand in it.
    Ready,
    /// A challenge under way: the elites it raised, by enemy id, until every one is dead.
    Challenge(Vec<u32>),
    /// A banner on its way: `carrier` must reach `to` within `left` seconds.
    Escort {
        carrier: PlayerId,
        to: V2,
        left: f32,
    },
    /// Used; it leaves the world in this many seconds.
    Spent(f32),
}

/// A shrine, altar, chest, banner or pot in the arena.
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

    /// Ready, or busy with a challenge or an escort: not yet done with.
    pub fn live(&self) -> bool {
        !matches!(self.state, InteractState::Spent(_))
    }
}

/// The gold a hero's next chest costs, having opened `opened` already: 20, 40, 70, 110, 160…
pub fn chest_price(opened: u32) -> u32 {
    20 + 15 * opened + 5 * opened * opened
}

/// How far a banner's destination lies from where it stood, in yards.
pub const ESCORT_DISTANCE: (f32, f32) = (55.0, 85.0);
/// How near its destination a carried banner has arrived.
pub const ESCORT_GOAL: f32 = 4.0;

/// The seconds a carrier at `speed` has to carry a banner `distance` yards: twice the straight
/// run, and a little more, for the horde in the way.
pub fn escort_time(distance: f32, speed: f32) -> f32 {
    distance / speed.max(1.0) * 2.0 + 6.0
}

fn ten() -> u32 {
    10
}

/// A kind a zone has standing about its arena: at most `count` at once, and how often it comes
/// up against the others (`weight`, 10 when left out).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Placement {
    pub kind: InteractableKind,
    pub count: u32,
    #[serde(default = "ten")]
    pub weight: u32,
}

/// A kind for a new interactable, by weight from the zone's placements still below their count
/// with `standing` already up; `None` if every kind is at its count.
pub(crate) fn pick_kind(
    zone: &ZoneDef,
    standing: &[InteractableKind],
    rng: &mut Rng,
) -> Option<InteractableKind> {
    let open: Vec<&Placement> = zone
        .interactables
        .iter()
        .filter(|p| (standing.iter().filter(|&&k| k == p.kind).count() as u32) < p.count)
        .collect();
    let total: u32 = open.iter().map(|p| p.weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = (rng.next_u64() % u64::from(total)) as u32;
    for p in open {
        if pick < p.weight {
            return Some(p.kind);
        }
        pick -= p.weight;
    }
    None
}

/// Tries at finding a spot clear of the others.
const SPOT_TRIES: u32 = 40;

/// How far apart interactables keep in `zone`'s arena.
pub(crate) fn spacing(zone: &ZoneDef) -> f32 {
    (zone.leash * 0.2).clamp(5.0, 14.0)
}

/// A spot `ring` yards (nearest, furthest) from `around`, inside the arena, as far as it can be
/// from every spot in `taken`: the first try clear of them by [`spacing`] wins.
pub(crate) fn spot(zone: &ZoneDef, around: V2, ring: (f32, f32), taken: &[V2], rng: &mut Rng) -> V2 {
    let center = V2::new(zone.center[0], zone.center[1]);
    let edge = zone.leash - 5.0;
    let mut best = (around, f32::MIN);
    for _ in 0..SPOT_TRIES {
        // Even over the ring's area, not bunched at its middle.
        let r = rng.range(ring.0 * ring.0, ring.1 * ring.1).sqrt();
        let mut at = around + V2::from_angle(rng.range(0.0, std::f32::consts::TAU), r);
        let off = at - center;
        if off.length() > edge {
            at = center + off.normalize_or_zero() * edge;
        }
        let clear = taken
            .iter()
            .map(|q| q.distance(at))
            .fold(f32::MAX, f32::min);
        if clear > best.1 {
            best = (at, clear);
        }
        if clear >= spacing(zone) {
            break;
        }
    }
    best.0
}

/// The run's first interactables: [`ZoneDef::shrines_at_once`] of them, kinds by weight, spread
/// over the arena between a ring kept clear around the centre (where the heroes start) and the
/// leash's edge.
pub(crate) fn scatter(zone: &ZoneDef, rng: &mut Rng) -> Vec<(InteractableKind, V2)> {
    let center = V2::new(zone.center[0], zone.center[1]);
    let inner = (zone.leash * 0.25).clamp(6.0, 20.0);
    let outer = (zone.leash - 5.0).max(inner + 1.0);
    let mut placed: Vec<(InteractableKind, V2)> = Vec::new();
    for _ in 0..zone.shrines_at_once {
        let kinds: Vec<InteractableKind> = placed.iter().map(|&(k, _)| k).collect();
        let Some(kind) = pick_kind(zone, &kinds, rng) else {
            break;
        };
        let taken: Vec<V2> = placed.iter().map(|&(_, p)| p).collect();
        placed.push((kind, spot(zone, center, (inner, outer), &taken, rng)));
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scatter_places_a_few_inside_the_arena_apart_and_clear_of_the_start() {
        let content = crate::content::tests::sample();
        let zone = &content.zones[0];
        let spots = scatter(zone, &mut Rng::new(5));
        assert_eq!(spots.len() as u32, zone.shrines_at_once);
        let center = V2::new(zone.center[0], zone.center[1]);
        for &(_, at) in &spots {
            let off = at.distance(center);
            assert!(off >= 20.0 - 1e-3 && off <= zone.leash - 5.0 + 1e-3, "{off}");
        }
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                assert!(a.1.distance(b.1) >= spacing(zone), "{a:?} and {b:?} crowd");
            }
        }
    }

    #[test]
    fn a_kind_at_its_count_is_not_picked() {
        let content = crate::content::tests::sample();
        let zone = &content.zones[0];
        let mut rng = Rng::new(3);
        // The sample allows one altar at once.
        let standing = [InteractableKind::CursedAltar];
        for _ in 0..200 {
            assert_ne!(
                pick_kind(zone, &standing, &mut rng),
                Some(InteractableKind::CursedAltar)
            );
        }
        let full: Vec<InteractableKind> = zone
            .interactables
            .iter()
            .flat_map(|p| std::iter::repeat_n(p.kind, p.count as usize))
            .collect();
        assert_eq!(pick_kind(zone, &full, &mut rng), None);
    }

    #[test]
    fn chests_cost_more_each_time() {
        assert_eq!(chest_price(0), 20);
        assert!((0..10).all(|n| chest_price(n + 1) > chest_price(n)));
    }

    #[test]
    fn a_longer_carry_gets_more_time() {
        assert!(escort_time(80.0, 7.0) > escort_time(60.0, 7.0));
        assert!(escort_time(60.0, 7.0) > 60.0 / 7.0);
    }
}
