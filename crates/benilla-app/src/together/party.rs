//! The party frames: WoW's own `PartyMemberFrame`s, top left under the player frame, showing the
//! run's other heroes. They are fed as a server's `SMSG_GROUP_LIST` and
//! `SMSG_PARTY_MEMBER_STATS` would feed them ([`GroupState`]); the frames then read each
//! member's portrait, health and level from its unit, as they do in WoW.

use std::collections::HashMap;

use benilla_protocol::messages::{member_status, GroupMemberEntry, PartyMemberStatsInfo};
use bevy::prelude::*;
use together_sim::{Hero, PlayerId};

use super::units::{self, UnitKind};
use super::view::HERO_SERIAL_BASE;
use super::{content, Phase, Sim};
use crate::ui_party::GroupState;

/// What the frames were last given: the roster (who, and whether standing), and each member's
/// numbers, so the group list and stats are sent again only when they change.
#[derive(Default)]
pub(super) struct Fed {
    roster: Vec<(PlayerId, String, bool)>,
    stats: HashMap<u64, (u16, u16, u32)>,
}

fn hero_guid(player: PlayerId) -> u64 {
    units::unit_guid(UnitKind::Player, HERO_SERIAL_BASE + u64::from(player))
}

fn status(hero: &Hero) -> u8 {
    if hero.alive {
        member_status::ONLINE
    } else {
        member_status::ONLINE | member_status::DEAD
    }
}

/// During a co-op run, the other heroes as our party, the host (player 0) its leader; otherwise
/// no party.
pub(super) fn feed(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    mut group: ResMut<GroupState>,
    mut fed: Local<Fed>,
) {
    let sim = sim.filter(|s| *phase.get() == Phase::Playing && s.world.heroes.len() > 1);
    let Some(sim) = sim else {
        if !fed.roster.is_empty() {
            group.clear_session();
            *fed = Fed::default();
        }
        return;
    };
    let world = &sim.world;
    let mut others: Vec<&Hero> = world.heroes.iter().filter(|h| h.player != sim.me).collect();
    others.sort_by_key(|h| h.player);

    let roster: Vec<(PlayerId, String, bool)> = others
        .iter()
        .map(|h| (h.player, h.name.clone(), h.alive))
        .collect();
    if roster != fed.roster {
        let members = others
            .iter()
            .map(|h| GroupMemberEntry {
                name: h.name.clone(),
                guid: hero_guid(h.player),
                status: status(h),
                flags: 0,
            })
            .collect();
        group.apply_list(0, 0, members, hero_guid(0), None, Some(hero_guid(sim.me)));
        fed.roster = roster;
        fed.stats.clear();
    }

    let content = content();
    for h in others {
        let guid = hero_guid(h.player);
        let numbers = (h.hp.ceil() as u16, h.max_hp.ceil() as u16, world.level);
        if fed.stats.get(&guid) == Some(&numbers) {
            continue;
        }
        fed.stats.insert(guid, numbers);
        group.apply_stats(
            guid,
            true,
            PartyMemberStatsInfo {
                status: Some(status(h)),
                cur_hp: Some(numbers.0),
                max_hp: Some(numbers.1),
                level: Some(world.level.min(u32::from(u16::MAX)) as u16),
                power_type: Some(content.classes[h.class].power),
                cur_power: Some(100),
                max_power: Some(100),
                ..Default::default()
            },
        );
    }
}
