//! The inn's NPCs (spec §8.4) at their places: each a character of `hub.ron`'s race and sex in a
//! class's starting outfit, named, standing where the hub says. A right-click on one within reach
//! opens their panel ([`InnPanel`]): the banker's stash and stash station, the trader's goods, a
//! giver's contracts, a keeper's station. Walking away closes it.

use benilla_world::interact::WorldRightClick;
use bevy::prelude::*;
use nightfall_sim::{NpcDef, NpcRole, V2};

use super::inn::InnBody;
use super::session::InnState;
use super::stash_panel::StashOpen;
use super::units::{self, UnitKind, UnitSpec};
use super::{content, dress, inn};
use crate::names::NameCache;
use crate::net::GuidIndex;

/// NPCs' guid serials start here.
const NPC_SERIAL_BASE: u64 = 0x100;
/// How near an NPC we must stand for their panel to open and stay open, yards.
pub(super) const PANEL_REACH: f32 = 6.0;

/// Which entity draws which NPC (by index into the inn's NPCs).
#[derive(Resource, Default)]
pub(super) struct NpcViews {
    pub drawn: Vec<(usize, Entity)>,
}

/// The NPC whose panel is open.
#[derive(Resource, Default)]
pub(super) struct InnPanel(pub Option<usize>);

/// The NPC an entity draws.
pub(super) fn npc_at(drawn: &[(usize, Entity)], e: Entity) -> Option<usize> {
    drawn.iter().find(|(_, d)| *d == e).map(|(i, _)| *i)
}

/// Whether a hero standing at `pos` is near enough to `npc` to use their panel.
fn within_reach(pos: V2, npc: &NpcDef) -> bool {
    pos.distance(npc.at.ground()) <= PANEL_REACH
}

/// The panel a right-click on NPC `i` opens, if the hero (at `pos`, once known) is within reach;
/// and whether the stash panel opens with it (the banker's). `None`: the click does nothing.
fn opened_by_click(npcs: &[NpcDef], i: usize, pos: Option<V2>) -> Option<(usize, bool)> {
    let npc = npcs.get(i)?;
    within_reach(pos?, npc).then_some((i, npc.role == NpcRole::Banker))
}

/// Whether the open panel of NPC `open` stays open with the hero at `pos`: walking out of reach
/// closes it. An unknown position (no hero yet) changes nothing.
fn stays_open(npcs: &[NpcDef], open: usize, pos: Option<V2>) -> bool {
    match (npcs.get(open), pos) {
        (Some(npc), Some(pos)) => within_reach(pos, npc),
        (None, _) => false,
        (_, None) => true,
    }
}

/// Every NPC drawn once the body tables are in, once per visit to the inn.
pub(super) fn spawn(
    mut commands: Commands,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut views: ResMut<NpcViews>,
) {
    if !views.drawn.is_empty() {
        return;
    }
    let Some(cc) = char_create.as_deref() else {
        return;
    };
    let content = content();
    let inn = inn();
    for (i, n) in inn.npcs.iter().enumerate() {
        let Some(display) = cc.0.body_display(n.body.race, n.body.sex) else {
            continue;
        };
        let outfit = cc.0.start_outfit(n.body.race, n.body.class_id, n.body.sex);
        let worn = dress::wear(&mut items, &dress::pieces(outfit, &[]));
        let serial = NPC_SERIAL_BASE + i as u64;
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                kind: UnitKind::Player,
                serial,
                display,
                scale: 1.0,
                level: content.rules.hero_level,
                faction: units::FACTION_FRIENDLY,
                bytes0: u32::from(n.body.race)
                    | u32::from(n.body.class_id) << 8
                    | u32::from(n.body.sex) << 16,
                weapons: &[],
                sheath: 0,
                extra: &worn,
            },
            units::wow_to_bevy([n.at.x, n.at.y, n.at.z]),
            units::yaw_of(V2::from_angle(n.facing.to_radians(), 1.0)),
        );
        commands.entity(e).insert(InnBody);
        let guid = units::unit_guid(UnitKind::Player, serial);
        index.0.insert(guid, e);
        names.insert_player(guid, n.name.clone(), None);
        views.drawn.push((i, e));
    }
}

/// A right-click on an NPC within reach opens their panel (the banker's opens the stash too).
pub(super) fn right_click(
    mut clicks: MessageReader<WorldRightClick>,
    press: Res<crate::target::PressPick>,
    views: Res<NpcViews>,
    state: Res<InnState>,
    mut panel: ResMut<InnPanel>,
    mut stash_open: ResMut<StashOpen>,
) {
    if clicks.read().last().is_none() {
        return;
    }
    let Some(target) = press.hovered.any().or(press.object.target) else {
        return;
    };
    let Some(i) = npc_at(&views.drawn, target) else {
        return;
    };
    if let Some((i, stash)) = opened_by_click(&inn().npcs, i, state.pose.map(|p| p.pos)) {
        panel.0 = Some(i);
        stash_open.0 = stash;
    }
}

/// Walking away from the NPC closes their panel.
pub(super) fn walk_away(
    state: Res<InnState>,
    mut panel: ResMut<InnPanel>,
    mut stash_open: ResMut<StashOpen>,
) {
    let Some(i) = panel.0 else {
        return;
    };
    if !stays_open(&inn().npcs, i, state.pose.map(|p| p.pos)) {
        panel.0 = None;
        stash_open.0 = false;
    }
}

/// The inn left: its NPCs went with its bodies; nothing is open.
pub(super) fn forget(
    mut views: ResMut<NpcViews>,
    mut panel: ResMut<InnPanel>,
    mut stash_open: ResMut<StashOpen>,
) {
    *views = NpcViews::default();
    panel.0 = None;
    stash_open.0 = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entity_names_its_npc() {
        let mut world = bevy::ecs::world::World::new();
        let (a, b, c) = (
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
        );
        let drawn = [(0, a), (3, b)];
        assert_eq!(npc_at(&drawn, b), Some(3));
        assert_eq!(npc_at(&drawn, c), None);
    }

    /// The shipped NPCs, with the banker's and the trader's indices.
    fn npcs() -> (Vec<NpcDef>, usize, usize) {
        let inn = super::super::shipped_inn();
        let find = |role| inn.npcs.iter().position(|n| n.role == role).unwrap();
        (inn.npcs.clone(), find(NpcRole::Banker), find(NpcRole::Trader))
    }

    /// A spot `d` yards east of the NPC.
    fn beside(npc: &NpcDef, d: f32) -> Option<V2> {
        let g = npc.at.ground();
        Some(V2::new(g.x, g.y + d))
    }

    #[test]
    fn a_click_opens_a_panel_only_within_reach() {
        let (npcs, banker, trader) = npcs();
        // The banker's opens with the stash, the trader's without.
        let near = beside(&npcs[banker], PANEL_REACH - 1.0);
        assert_eq!(opened_by_click(&npcs, banker, near), Some((banker, true)));
        let near = beside(&npcs[trader], PANEL_REACH - 1.0);
        assert_eq!(opened_by_click(&npcs, trader, near), Some((trader, false)));
        // Too far, no hero yet, or no such NPC: nothing opens.
        let far = beside(&npcs[trader], PANEL_REACH + 1.0);
        assert_eq!(opened_by_click(&npcs, trader, far), None);
        assert_eq!(opened_by_click(&npcs, trader, None), None);
        assert_eq!(opened_by_click(&npcs, npcs.len(), near), None);
    }

    #[test]
    fn a_panel_closes_when_its_hero_walks_away() {
        let (npcs, _, trader) = npcs();
        assert!(stays_open(&npcs, trader, beside(&npcs[trader], PANEL_REACH - 0.5)));
        assert!(!stays_open(&npcs, trader, beside(&npcs[trader], PANEL_REACH + 0.5)));
        // No position yet changes nothing; an NPC that is not there is not open.
        assert!(stays_open(&npcs, trader, None));
        assert!(!stays_open(&npcs, npcs.len(), None));
    }
}
