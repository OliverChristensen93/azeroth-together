//! The raid (spec §8): what each hero carries and does with it, the containers on the map and
//! what they hold, the exits and extracting, deaths and what they cost, and each hero's outcome.
//! The host's: a client sees its own backpack, the containers near it and the one it has open
//! ([`LootView`]) in its snapshot, and is told its outcome.

mod acts;
mod containers;
mod extract;
#[cfg(test)]
mod tests;

pub(crate) use containers::{in_reach, search_of};

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    Content, EquipSlot, Inventory, ItemKind, Place, PlayerId, RunRules, Stack, World, V2, V3,
};

/// What a hero carries, as every client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Carry {
    /// The weight carried (spec §8.2).
    pub load: f32,
    /// The lit lantern, an index into the content's items.
    pub lantern: Option<u16>,
    /// What they wear, for every client to dress them in.
    pub worn: Vec<(EquipSlot, u16)>,
    /// Seconds in an open exit's ring (spec §8.1).
    pub extracting: f32,
    /// The last act the host took from them ([`ActPress::seq`]).
    pub acted: u8,
}

/// Something a player does with what they carry or find.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    /// Search a container within reach, or open one already searched.
    Open(u32),
    /// Stop searching, and close what is open.
    Close,
    /// The stack in this slot of the open container, into the backpack.
    Take(u16),
    /// Everything in the open container that fits.
    TakeAll,
    /// The stack at this place into the open container.
    Put(Place),
    Move {
        from: Place,
        to: Place,
    },
    /// The stack at this place onto the ground, where friends can pick it up.
    Drop(Place),
    /// Light or put out a lantern, drink a potion.
    Use(Place),
}

/// An act, repeated in every input until the host takes it ([`Carry::acted`] says when); `seq`
/// tells it from the last one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActPress {
    pub seq: u8,
    pub act: Act,
}

/// What holds a container's things.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Holder {
    /// A chest, crate or grave the content placed: its kind, an index into the loot's kinds.
    Placed(u16),
    /// A dead enemy, by id.
    Body(u32),
    /// A fallen hero.
    Hero(PlayerId),
    /// A hero's body from their last raid (spec §8.2's one recovery), seen only by them.
    Remains(PlayerId),
    /// Things dropped on the ground.
    Pile,
}

/// A container near the player, for drawing and clicking.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerMark {
    pub id: u32,
    pub holder: Holder,
    pub at: V2,
    pub z: Option<f32>,
    pub searched: bool,
    /// Searched, and nothing in it: an unsearched container does not say.
    pub empty: bool,
    pub locked: bool,
}

/// The container a player has open, or is searching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenView {
    pub id: u32,
    /// Its slots, once searched.
    pub stacks: Vec<Option<Stack>>,
    /// While searching: seconds left, and the whole search.
    pub search: Option<(f32, f32)>,
}

/// An exit as the raid has it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExitMark {
    /// Index into the map's exits.
    pub exit: u8,
    pub open: bool,
    /// Seconds until it opens; `None` when it stays shut this raid.
    pub opens_in: Option<f32>,
}

/// What a player sees of the raid's loot: the containers near them, the one they have open, the
/// exits.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LootView {
    pub marks: Vec<ContainerMark>,
    pub open: Option<OpenView>,
    pub exits: Vec<ExitMark>,
}

/// A body left for its player's next raid on the map (spec §8.2, `everything_one_recovery`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recovery {
    /// The map's key.
    pub map: String,
    pub at: V3,
    pub items: Vec<Stack>,
}

/// What a player brings into a raid (spec §8.3: the host trusts it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loadout {
    pub inventory: Inventory,
    /// Their remains from the last raid, when there are any.
    pub recovery: Option<Recovery>,
}

/// How a hero's raid ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fate {
    /// Out, by this exit.
    Extracted {
        exit: String,
    },
    Died,
    /// The raid ended around them (it was abandoned): counted as a death with no body.
    Missing,
}

/// One hero's raid, as its end screen and its player's stash take it (spec §8.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeroOutcome {
    pub player: PlayerId,
    pub name: String,
    pub fate: Fate,
    /// Seconds into the raid it ended for them.
    pub time: f32,
    pub kills: u32,
    /// What they keep: everything carried out, or what a death left them.
    pub carried: Inventory,
    /// What they took back from their remains, for the stash.
    pub recovered: Vec<Stack>,
    /// The body they left for their next raid.
    pub recovery: Option<Recovery>,
}

/// One exit in one raid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExitPlan {
    pub open: bool,
    /// Seconds into the raid it opens.
    pub opens_at: f32,
}

/// What makes this raid this raid (spec §8.1): the seed its loot is rolled from, the entry, and
/// which exits open when. Sub-project 6 rolls the exits, the weather and the signature monsters
/// here; until then [`RaidRoll::standard`] is every raid.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RaidRoll {
    pub seed: u64,
    /// Index into the map's entries.
    pub entry: usize,
    /// One per map exit.
    pub exits: Vec<ExitPlan>,
}

impl RaidRoll {
    /// The first entry, every exit open, the late ones at `cart_opens_at`.
    pub fn standard(content: &Content, seed: u64) -> Self {
        let cart = content.rules.run.cart_opens_at;
        Self {
            seed,
            entry: 0,
            exits: content
                .map
                .exits
                .iter()
                .map(|e| ExitPlan {
                    open: true,
                    opens_at: if e.late { cart } else { 0.0 },
                })
                .collect(),
        }
    }
}

/// Weight's cost (spec §8.2): the share of normal speed, and how many times further footsteps
/// carry. Nothing up to `weight_free`, all of it from `weight_full`, in a straight line between;
/// the floor is above 0, so a hero is never stuck.
pub fn burden(load: f32, r: &RunRules) -> (f32, f32) {
    let over = ((load - r.weight_free) / (r.weight_full - r.weight_free)).clamp(0.0, 1.0);
    // A load that is not a number weighs nothing (the content refuses a zero-width band).
    let over = if over.is_nan() { 0.0 } else { over };
    (
        1.0 - over * (1.0 - r.weight_slow_floor),
        1.0 + over * (r.weight_noise_max - 1.0),
    )
}

/// The drinks in `inv`'s backpack, the ones a hero can drink (not the safe slots or what they
/// wear): what Conjure Water counts against `max_drinks` and what
/// [`HeroFight::drinks`](crate::HeroFight::drinks) reports (capped there).
pub(crate) fn drinks_in(inv: &Inventory, content: &Content) -> u32 {
    inv.stacks()
        .iter()
        .filter(|(at, s)| {
            matches!(at, Place::Pack(_))
                && content
                    .items
                    .get(usize::from(s.item))
                    .is_some_and(|d| d.kind == ItemKind::Drink)
        })
        .map(|(_, s)| u32::from(s.count))
        .sum()
}

/// A container in the host's world.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Container {
    pub(crate) id: u32,
    pub(crate) holder: Holder,
    pub(crate) at: V2,
    pub(crate) z: Option<f32>,
    pub(crate) stacks: Vec<Option<Stack>>,
    pub(crate) searched: bool,
    /// The key item it needs.
    pub(crate) lock: Option<u16>,
}

/// A search under way.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Search {
    pub(crate) player: PlayerId,
    pub(crate) id: u32,
    pub(crate) left: f32,
    pub(crate) total: f32,
    /// Where the hero stood as it began: walking off stops it.
    pub(crate) from: V2,
    /// Seconds to the next time it is heard.
    pub(crate) noise_in: f32,
}

/// A hero's extract count.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Exiting {
    pub(crate) player: PlayerId,
    pub(crate) t: f32,
    /// Health at the last step: less now is a blow, which starts the count over.
    pub(crate) last_hp: f32,
}

/// The raid's host-only state: never sent whole.
#[derive(Clone, Debug, Default)]
pub(crate) struct Raid {
    pub(crate) roll: RaidRoll,
    pub(crate) packs: Vec<(PlayerId, Inventory)>,
    pub(crate) containers: Vec<Container>,
    pub(crate) next_id: u32,
    pub(crate) searches: Vec<Search>,
    pub(crate) opened: Vec<(PlayerId, u32)>,
    /// Each player's last act taken.
    pub(crate) acts: Vec<(PlayerId, u8)>,
    /// When each player may drink a potion again.
    pub(crate) potions: Vec<(PlayerId, f32)>,
    /// Every hero's extract count.
    pub(crate) exiting: Vec<Exiting>,
    /// Enemies killed, by the hero who killed them ([`World::tally_kills`]).
    pub(crate) kills: Vec<(PlayerId, u32)>,
    /// Enemies whose body has been made.
    pub(crate) bodied: BTreeSet<u32>,
    /// Fallen heroes and their body's container (0 for none).
    pub(crate) fallen: Vec<(PlayerId, u32)>,
    /// What each player took back from their remains.
    pub(crate) recovered: Vec<(PlayerId, Vec<Stack>)>,
    /// Brought stacks a dead hero kept but whose backpack no longer holds them (under `found_only`
    /// a found bag goes with them): theirs in their outcome, never on the body.
    pub(crate) spilled: Vec<(PlayerId, Vec<Stack>)>,
    /// Every hero's settled outcome, in the order they were settled.
    pub(crate) outcomes: Vec<HeroOutcome>,
    /// Outcomes before this index have been handed out.
    pub(crate) taken: usize,
    /// Whether any hero has been in the raid: an empty world is not an ended raid.
    pub(crate) had_heroes: bool,
    /// Every hero is out or dead, or the raid was abandoned.
    pub(crate) over: bool,
    /// Events already tallied for kills ([`World::tally_kills`]).
    pub(crate) counted: usize,
    /// A client's mirror: the loot view its host sent.
    pub(crate) mirror: Option<LootView>,
}

impl Raid {
    pub(crate) fn pack(&self, player: PlayerId) -> Option<&Inventory> {
        self.packs
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, i)| i)
    }

    /// One of the first `kind` (food or drink) in `player`'s backpack eaten: the share of health
    /// or mana it gives back. The safe slots are not eaten from.
    pub(crate) fn eat(
        &mut self,
        player: PlayerId,
        kind: ItemKind,
        content: &Content,
    ) -> Option<f32> {
        let inv = self
            .packs
            .iter_mut()
            .find(|(p, _)| *p == player)
            .map(|(_, i)| i)?;
        let at = inv.first_of(kind, content)?;
        let s = inv.take_one(at)?;
        Some(content.items.get(usize::from(s.item))?.restore)
    }

    /// Whether `player`'s backpack holds a meal for them: food, or for a mana user a drink as
    /// well. A drink alone is no meal for a hero who has no mana to drink it for.
    pub(crate) fn has_meal(&self, player: PlayerId, mana_user: bool, content: &Content) -> bool {
        self.pack(player).is_some_and(|i| {
            i.first_of(ItemKind::Food, content).is_some()
                || (mana_user && i.first_of(ItemKind::Drink, content).is_some())
        })
    }

    /// `player`'s inventory, an empty one made if they have none yet.
    pub(crate) fn pack_mut(&mut self, player: PlayerId, content: &Content) -> &mut Inventory {
        let i = match self.packs.iter().position(|(p, _)| *p == player) {
            Some(i) => i,
            None => {
                self.packs.push((player, Inventory::new(content)));
                self.packs.len() - 1
            }
        };
        &mut self.packs[i].1
    }

    pub(crate) fn take_pack(&mut self, player: PlayerId) -> Option<Inventory> {
        let i = self.packs.iter().position(|(p, _)| *p == player)?;
        Some(self.packs.remove(i).1)
    }

    pub(crate) fn last_act(&self, player: PlayerId) -> Option<u8> {
        self.acts
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, s)| *s)
    }

    pub(crate) fn set_last_act(&mut self, player: PlayerId, seq: u8) {
        match self.acts.iter_mut().find(|(p, _)| *p == player) {
            Some(a) => a.1 = seq,
            None => self.acts.push((player, seq)),
        }
    }

    pub(crate) fn potion_ready(&self, player: PlayerId) -> f32 {
        self.potions
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(0.0, |(_, t)| *t)
    }

    pub(crate) fn set_potion_ready(&mut self, player: PlayerId, at: f32) {
        match self.potions.iter_mut().find(|(p, _)| *p == player) {
            Some(t) => t.1 = at,
            None => self.potions.push((player, at)),
        }
    }

    pub(crate) fn open_of(&self, player: PlayerId) -> Option<u32> {
        self.opened
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, id)| *id)
    }

    /// `player` stops searching, and closes what they have open.
    pub(crate) fn close(&mut self, player: PlayerId) {
        self.opened.retain(|(p, _)| *p != player);
        self.searches.retain(|s| s.player != player);
    }

    pub(crate) fn recover(&mut self, player: PlayerId, stacks: Vec<Stack>) {
        match self.recovered.iter_mut().find(|(p, _)| *p == player) {
            Some(r) => r.1.extend(stacks),
            None => self.recovered.push((player, stacks)),
        }
    }

    pub(crate) fn take_recovered(&mut self, player: PlayerId) -> Vec<Stack> {
        match self.recovered.iter().position(|(p, _)| *p == player) {
            Some(i) => self.recovered.remove(i).1,
            None => Vec::new(),
        }
    }

    /// What `player` kept but their backpack no longer holds, taken for their outcome.
    pub(crate) fn take_spilled(&mut self, player: PlayerId) -> Vec<Stack> {
        match self.spilled.iter().position(|(p, _)| *p == player) {
            Some(i) => self.spilled.remove(i).1,
            None => Vec::new(),
        }
    }

    pub(crate) fn add_kill(&mut self, player: PlayerId) {
        match self.kills.iter_mut().find(|(p, _)| *p == player) {
            Some(k) => k.1 += 1,
            None => self.kills.push((player, 1)),
        }
    }

    pub(crate) fn kills_of(&self, player: PlayerId) -> u32 {
        self.kills
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(0, |(_, k)| *k)
    }

    /// `player` is gone from the raid: nothing they were doing goes on.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.searches.retain(|s| s.player != player);
        self.opened.retain(|(p, _)| *p != player);
        self.acts.retain(|(p, _)| *p != player);
        self.potions.retain(|(p, _)| *p != player);
        self.exiting.retain(|e| e.player != player);
        self.fallen.retain(|(p, _)| *p != player);
        self.spilled.retain(|(p, _)| *p != player);
    }

    /// What `player` has open or is searching.
    pub(crate) fn open_view(&self, player: PlayerId) -> Option<OpenView> {
        if let Some(s) = self.searches.iter().find(|s| s.player == player) {
            return Some(OpenView {
                id: s.id,
                stacks: Vec::new(),
                search: Some((s.left.max(0.0), s.total)),
            });
        }
        let id = self.open_of(player)?;
        let c = self.containers.iter().find(|c| c.id == id)?;
        Some(OpenView {
            id,
            stacks: c.stacks.clone(),
            search: None,
        })
    }
}

impl World {
    /// This raid is `roll`'s: its entry, its exits, its loot rolled from its seed. Called before
    /// the heroes are added, so they start at its entry.
    pub fn begin_raid(&mut self, roll: RaidRoll) -> Result<(), String> {
        let content = self.content().clone();
        if roll.entry >= content.map.entries.len() {
            return Err(format!("no entry {} on {}", roll.entry, content.map.key));
        }
        if roll.exits.len() != content.map.exits.len() {
            return Err(format!(
                "{} exits planned for {}'s {}",
                roll.exits.len(),
                content.map.key,
                content.map.exits.len()
            ));
        }
        self.raid = Raid::new(&content, roll);
        Ok(())
    }

    /// `player`, already added, brings `loadout` (spec §8.3: the host trusts it, once it is made
    /// right for this content): their gear, backpack and safe slots, full health with what they
    /// wear, and their remains from their last raid when it was on this map. What has no room is
    /// returned, and left out.
    pub fn give_loadout(&mut self, player: PlayerId, loadout: &Loadout) -> Vec<Stack> {
        let content = self.content().clone();
        let mut inv = loadout.inventory.clone();
        let left = inv.sanitize(&content);
        *self.raid.pack_mut(player, &content) = inv;
        let remains = loadout
            .recovery
            .as_ref()
            .filter(|r| r.map == content.map.key && r.at.is_finite());
        if let Some(r) = remains {
            let items: Vec<Stack> = r
                .items
                .iter()
                .copied()
                .filter(|s| usize::from(s.item) < content.items.len() && s.count > 0)
                .collect();
            let at = content.map.bounds.clamp(r.at.ground());
            let slots = items.len();
            self.raid.add_container(
                Holder::Remains(player),
                at,
                Some(r.at.z),
                items,
                slots,
                true,
                None,
            );
        }
        self.refresh_carry(player);
        if let Some(hi) = self.hero_slot(player) {
            let h = &mut self.heroes[hi];
            h.hp = h.max_hp;
        }
        left
    }

    /// `player`'s inventory: the host's, or on a client's mirror its own as the host sent it.
    pub fn pack(&self, player: PlayerId) -> Option<&Inventory> {
        self.raid.pack(player)
    }

    /// The raid's loot as `player` sees it: the containers within the snapshot reach of their hero
    /// ([`World::reach`]: the view distance and [`crate::KEEP_EXTRA`] past it, which is also what
    /// [`World::in_reach`] and their snapshot's enemies use; their own remains, nobody else's), the
    /// one they have open, and the exits; every container for `None`. A client's mirror answers
    /// with what its host sent.
    pub fn loot_view(&self, player: Option<PlayerId>) -> LootView {
        if let Some(view) = &self.raid.mirror {
            return view.clone();
        }
        let content = self.content();
        let near = player.and_then(|p| self.hero(p)).map(|h| h.pos);
        // The same reach as `World::in_reach`, the events and the snapshot's enemies.
        let reach = self.reach();
        let marks = self
            .raid
            .containers
            .iter()
            .filter(|c| match c.holder {
                Holder::Remains(owner) => player.is_none_or(|p| p == owner),
                _ => true,
            })
            .filter(|c| near.is_none_or(|p| p.distance(c.at) <= reach))
            .map(|c| ContainerMark {
                id: c.id,
                holder: c.holder,
                at: c.at,
                z: c.z,
                searched: c.searched,
                // An unsearched container does not say whether it is empty.
                empty: c.searched && c.stacks.iter().all(Option::is_none),
                locked: c.lock.is_some(),
            })
            .collect();
        let open = player.and_then(|p| self.raid.open_view(p));
        let exits = (0..content.map.exits.len())
            .map(|i| {
                let plan = self.raid.roll.exits.get(i).copied().unwrap_or(ExitPlan {
                    open: false,
                    opens_at: 0.0,
                });
                ExitMark {
                    exit: i as u8,
                    open: plan.open && self.elapsed >= plan.opens_at,
                    opens_in: plan.open.then(|| (plan.opens_at - self.elapsed).max(0.0)),
                }
            })
            .collect();
        LootView { marks, open, exits }
    }

    /// What `player`'s worn gear multiplies their blows and spells by.
    /// The content refuses a damage bonus that is not above -1, so this is never negative; it is
    /// held to that anyway, and to a number.
    pub(crate) fn gear_damage(&self, player: PlayerId) -> f32 {
        let content = self.content();
        let bonus = self
            .raid
            .pack(player)
            .map_or(0.0, |i| i.bonus(content).damage);
        if bonus.is_finite() {
            (1.0 + bonus).max(0.0)
        } else {
            1.0
        }
    }

    /// `player` leaves the raid without an outcome (they left the session): what a death would
    /// take is dropped where they stood, for their friends, and nothing of theirs goes on. What a
    /// death would leave them ([`crate::Death::spill`] included) goes with them: a hero who left
    /// has no outcome, so nothing reaches their stash.
    pub(crate) fn raid_leave(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let at = self.hero(player).map(|h| V3::at(h.pos, h.z));
        if let (Some(at), Some(mut inv)) = (at, self.raid.take_pack(player)) {
            for s in inv
                .split_for_death(content.rules.run.death_loss, &content)
                .lost
            {
                self.raid.drop_on_ground(at, s, &content);
            }
        }
        self.raid.forget(player);
    }
}
