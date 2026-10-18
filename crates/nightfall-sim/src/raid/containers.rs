//! Containers: the chests, crates and graves the content places, the bodies enemies and heroes
//! leave, a hero's remains from their last raid, and things dropped on the ground.

use super::{Container, Holder, Raid, RaidRoll};
use crate::{Content, Loudness, Stack, V2, V3};

/// How far up or down a container may be from a hero and still be in reach, yards: a chest in a
/// crypt is not opened from the hill above it.
pub(crate) const REACH_UP: f32 = 3.0;
/// Things dropped this near a pile join it, yards.
const PILE_JOIN: f32 = 1.5;

/// Whether a hero at `pos` (feet at `z`) reaches a container at `at` (at height `cz`, when it is
/// known).
pub(crate) fn in_reach(pos: V2, z: f32, at: V2, cz: Option<f32>, reach: f32) -> bool {
    pos.distance(at) <= reach && cz.is_none_or(|cz| (z - cz).abs() <= REACH_UP)
}

/// How long searching what `holder` holds takes, and how loud it is (spec §5.2).
pub(crate) fn search_of(holder: Holder, content: &Content) -> (f32, Option<Loudness>) {
    match holder {
        Holder::Placed(k) => {
            let k = &content.loot.kinds[usize::from(k)];
            (k.search, Some(k.sound))
        }
        Holder::Body(_) | Holder::Hero(_) => (content.rules.run.search_body, Some(Loudness::Open)),
        Holder::Remains(_) | Holder::Pile => (0.0, None),
    }
}

impl Raid {
    /// A raid as `roll` has it.
    pub(crate) fn new(_content: &Content, roll: RaidRoll) -> Self {
        Self {
            roll,
            next_id: 1,
            ..Self::default()
        }
    }

    /// A new container holding `stacks`, with at least `slots` slots; its id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_container(
        &mut self,
        holder: Holder,
        at: V2,
        z: Option<f32>,
        stacks: Vec<Stack>,
        slots: usize,
        searched: bool,
        lock: Option<u16>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut held: Vec<Option<Stack>> = stacks.into_iter().map(Some).collect();
        if held.len() < slots {
            held.resize(slots, None);
        }
        self.containers.push(Container {
            id,
            holder,
            at,
            z,
            stacks: held,
            searched,
            lock,
        });
        id
    }

    /// `stack` on the ground at `at`: first onto an equal stack (the same item, found or brought)
    /// with room in a pile there, then into an empty slot of a pile there, or a new pile. So
    /// drops of one thing build up in one stack, not one slot each.
    pub(crate) fn drop_on_ground(&mut self, at: V3, mut stack: Stack, content: &Content) {
        let here = at.ground();
        let most = content
            .items
            .get(usize::from(stack.item))
            .map_or(u16::MAX, |d| d.stack.max(1));
        for c in self
            .containers
            .iter_mut()
            .filter(|c| c.holder == Holder::Pile && c.at.distance(here) <= PILE_JOIN)
        {
            for s in c.stacks.iter_mut().flatten() {
                if s.item == stack.item && s.found == stack.found && s.count < most {
                    let n = (most - s.count).min(stack.count);
                    s.count += n;
                    stack.count -= n;
                }
            }
        }
        if stack.count == 0 {
            return;
        }
        let near = self.containers.iter().position(|c| {
            c.holder == Holder::Pile
                && c.at.distance(here) <= PILE_JOIN
                && c.stacks.iter().any(Option::is_none)
        });
        match near {
            Some(i) => {
                let c = &mut self.containers[i];
                let free = c.stacks.iter().position(Option::is_none).expect("has room");
                c.stacks[free] = Some(stack);
            }
            None => {
                let slots = usize::from(content.rules.run.container_slots);
                self.add_container(
                    Holder::Pile,
                    here,
                    Some(at.z),
                    vec![stack],
                    slots,
                    true,
                    None,
                );
            }
        }
    }
}
