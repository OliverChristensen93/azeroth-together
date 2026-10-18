//! What heroes do with what they carry and find (spec §8.2): wear, move, use and drop things,
//! open and search containers, take from them and put into them. Each act comes once
//! ([`ActPress`]): it rides in every input until the host takes it, so an input the host
//! replaced before its step loses nothing, and the same act seen again does nothing.

use super::{in_reach, search_of, Act, ActPress, Holder, Search};
use crate::{
    Content, EquipSlot, ItemKind, ItemUse, Place, PlayerId, PlayerInput, Stack, World, V3,
};

impl World {
    /// `player`'s act in `input`, unless it is the one already taken. A hero who is not up is
    /// answered, so their client moves on, but does nothing.
    pub(crate) fn take_act(&mut self, player: PlayerId, input: &PlayerInput) {
        let Some(ActPress { seq, act }) = input.act else {
            return;
        };
        if self.raid.last_act(player) == Some(seq) {
            return;
        }
        self.raid.set_last_act(player, seq);
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        self.heroes[hi].carry.acted = seq;
        if !self.heroes[hi].fight.up() {
            return;
        }
        let content = self.content().clone();
        match act {
            Act::Move { from, to } => {
                let _ = self
                    .raid
                    .pack_mut(player, &content)
                    .move_to(from, to, &content);
            }
            Act::Use(at) => self.use_item(player, hi, at, &content),
            Act::Drop(at) => {
                let here = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
                if let Some(stack) = self.raid.pack_mut(player, &content).take(at) {
                    self.raid.drop_on_ground(here, stack, &content);
                }
            }
            Act::Open(id) => self.open(player, hi, id, &content),
            Act::Close => self.raid.close(player),
            Act::Take(slot) => self.take_from(player, Some(usize::from(slot)), &content),
            Act::TakeAll => self.take_from(player, None, &content),
            Act::Put(at) => self.put_into(player, at, &content),
        }
        self.refresh_carry(player);
    }

    /// A lantern lit or put out; a potion drunk, when the last one's wait is over.
    fn use_item(&mut self, player: PlayerId, hi: usize, at: Place, content: &Content) {
        let Some(stack) = self.raid.pack(player).and_then(|i| i.get(at)).copied() else {
            return;
        };
        let Some(def) = content.items.get(usize::from(stack.item)) else {
            return;
        };
        match def.kind {
            ItemKind::Lantern => {
                let c = &mut self.heroes[hi].carry;
                c.lantern = if c.lantern == Some(stack.item) {
                    None
                } else {
                    Some(stack.item)
                };
            }
            ItemKind::Potion => {
                let now = self.elapsed;
                let Some(effect) = def.on_use else {
                    return;
                };
                if self.raid.potion_ready(player) > now {
                    return;
                }
                if self.raid.pack_mut(player, content).take_one(at).is_none() {
                    return;
                }
                let h = &mut self.heroes[hi];
                match effect {
                    ItemUse::Heal(n) => h.hp = (h.hp + n).min(h.max_hp),
                    ItemUse::Mana(n) => h.fight.power = (h.fight.power + n).min(h.fight.max_power),
                }
                self.raid
                    .set_potion_ready(player, now + content.rules.run.potion_cooldown);
            }
            _ => {}
        }
    }

    /// Search container `id` within reach, or open it once searched: a locked one needs its key
    /// carried; a hero's own remains give everything back at once (spec §8.2).
    fn open(&mut self, player: PlayerId, hi: usize, id: u32, content: &Content) {
        let (pos, z) = (self.heroes[hi].pos, self.heroes[hi].z);
        let Some(ci) = self.raid.containers.iter().position(|c| c.id == id) else {
            return;
        };
        let (at, cz, holder, lock, searched) = {
            let c = &self.raid.containers[ci];
            (c.at, c.z, c.holder, c.lock, c.searched)
        };
        if !in_reach(pos, z, at, cz, content.rules.run.reach) {
            return;
        }
        if let Holder::Remains(owner) = holder {
            if owner == player {
                let c = self.raid.containers.remove(ci);
                self.raid
                    .recover(player, c.stacks.into_iter().flatten().collect());
            }
            return;
        }
        if let Some(key) = lock {
            let has = self
                .raid
                .pack(player)
                .is_some_and(|i| i.count_of(usize::from(key)) > 0);
            if !has {
                return;
            }
        }
        if self
            .raid
            .searches
            .iter()
            .any(|s| s.player == player && s.id == id)
        {
            return;
        }
        self.raid.close(player);
        let (total, _) = search_of(holder, content);
        if searched || total <= 0.0 {
            self.raid.containers[ci].searched = true;
            self.raid.opened.push((player, id));
        } else {
            self.raid.searches.push(Search {
                player,
                id,
                left: total,
                total,
                from: pos,
                noise_in: 0.0,
            });
        }
    }

    /// The stack in `slot` (or every stack) of the open container into the backpack, as found
    /// this raid; what does not fit stays.
    fn take_from(&mut self, player: PlayerId, slot: Option<usize>, content: &Content) {
        let Some(id) = self.raid.open_of(player) else {
            return;
        };
        let Some(ci) = self.raid.containers.iter().position(|c| c.id == id) else {
            return;
        };
        let slots: Vec<usize> = match slot {
            Some(s) => vec![s],
            None => (0..self.raid.containers[ci].stacks.len()).collect(),
        };
        for s in slots {
            let Some(stack) = self.raid.containers[ci].stacks.get(s).copied().flatten() else {
                continue;
            };
            let left = self.raid.pack_mut(player, content).add(
                Stack {
                    found: true,
                    ..stack
                },
                content,
            );
            self.raid.containers[ci].stacks[s] = left.map(|l| Stack {
                found: stack.found,
                ..l
            });
        }
    }

    /// The stack at `at` into the open container's first empty slot.
    fn put_into(&mut self, player: PlayerId, at: Place, content: &Content) {
        let Some(id) = self.raid.open_of(player) else {
            return;
        };
        let Some(ci) = self.raid.containers.iter().position(|c| c.id == id) else {
            return;
        };
        let Some(free) = self.raid.containers[ci]
            .stacks
            .iter()
            .position(Option::is_none)
        else {
            return;
        };
        let Some(stack) = self.raid.pack_mut(player, content).take(at) else {
            return;
        };
        self.raid.containers[ci].stacks[free] = Some(stack);
    }

    /// `player`'s carry after a change: the weight, what they wear, their lantern (out once it
    /// left them), the drinks they carry, and their health with what they wear. More health from
    /// gear arrives whole only to a hero at full health, so taking gear off and on heals nobody.
    pub(crate) fn refresh_carry(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let Some(inv) = self.raid.pack(player) else {
            return;
        };
        let load = inv.weight(&content);
        let worn: Vec<(EquipSlot, u16)> = EquipSlot::ALL
            .iter()
            .filter_map(|&s| inv.equip[s.index()].map(|st| (s, st.item)))
            .collect();
        let drinks: u32 = inv
            .stacks()
            .iter()
            .filter(|(_, s)| {
                content
                    .items
                    .get(usize::from(s.item))
                    .is_some_and(|d| d.kind == ItemKind::Drink)
            })
            .map(|(_, s)| u32::from(s.count))
            .sum();
        let bonus = inv.bonus(&content);
        let lantern_kept = self.heroes[hi]
            .carry
            .lantern
            .is_some_and(|l| inv.count_of(usize::from(l)) > 0);
        let h = &mut self.heroes[hi];
        h.carry.load = load;
        h.carry.worn = worn;
        if !lantern_kept {
            h.carry.lantern = None;
        }
        h.fight.drinks = drinks.min(u32::from(u8::MAX)) as u8;
        // Never below 1, whatever the gear says, so a hero is never born dead by it.
        let max = (content.classes[h.class].base_hp + bonus.health).max(1.0);
        if (max - h.max_hp).abs() > f32::EPSILON {
            let whole = h.hp >= h.max_hp;
            h.max_hp = max;
            if whole && h.fight.up() {
                h.hp = max;
            }
            h.hp = h.hp.min(max);
        }
    }
}
