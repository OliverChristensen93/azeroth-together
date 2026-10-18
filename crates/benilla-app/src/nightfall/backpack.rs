//! The raid's backpack and loot panels (spec §8.2): what our hero wears, their bags, backpack and
//! safe slots, and the container they have open. B opens the backpack (WoW's own bag windows stay
//! shut: their functions are quieted, since our things are not in WoW's bags); searching or
//! opening a container opens the loot panel. A click does the likely thing ([`smart_act`]): wear,
//! take off, light, drink, or put into the open container; Shift-click moves to or from a safe
//! slot; Ctrl-click drops on the ground. The host does it ([`Act`]).

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use nightfall_sim::{
    Act, Content, EquipSlot, Holder, Inventory, ItemKind, LootView, OpenView, Place, Stack, World,
};

use super::run::{LocalInput, Sim};
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL, SLOT};
use super::{content, Phase};

/// What a click on our stack at `at` does: Ctrl drops it, Shift moves it to or from a safe slot,
/// with a container open it goes in; else gear is worn (or taken off into the backpack), a bag is
/// worn in a free bag slot, a lantern lit, a potion drunk. Food and drink are for Eat and Drink.
pub(super) fn smart_act(
    inv: &Inventory,
    at: Place,
    container_open: bool,
    shift: bool,
    ctrl: bool,
    content: &Content,
) -> Option<Act> {
    let s = inv.get(at)?;
    let def = content.items.get(usize::from(s.item))?;
    let to = |to: Place| Some(Act::Move { from: at, to });
    if ctrl {
        return (!matches!(at, Place::Bag(_))).then_some(Act::Drop(at));
    }
    if shift {
        return match at {
            Place::Safe(_) => inv.first_empty(false).and_then(to),
            Place::Bag(_) => None,
            _ => inv.first_empty(true).and_then(to),
        };
    }
    if container_open && !matches!(at, Place::Bag(_)) {
        return Some(Act::Put(at));
    }
    match at {
        Place::Equip(_) | Place::Bag(_) => inv.first_empty(false).and_then(to),
        Place::Pack(_) | Place::Safe(_) => {
            if let Some(slot) = def.slot {
                return to(Place::Equip(slot));
            }
            match def.kind {
                ItemKind::Bag => {
                    let free = inv.bags.iter().position(Option::is_none).unwrap_or(0);
                    to(Place::Bag(free as u8))
                }
                ItemKind::Lantern | ItemKind::Potion => Some(Act::Use(at)),
                _ => None,
            }
        }
    }
}

/// The one act a click on `slot` means, if any: a stack in the open container is taken; one of
/// ours does [`smart_act`] (a container receives only once it is searched). An empty slot means
/// nothing, so nothing is queued for it.
pub(super) fn click_act(
    slot: PackSlot,
    inv: Option<&Inventory>,
    open: Option<&OpenView>,
    shift: bool,
    ctrl: bool,
    content: &Content,
) -> Option<Act> {
    match slot {
        PackSlot::Loot(n) => open?
            .stacks
            .get(usize::from(n))
            .copied()
            .flatten()
            .map(|_| Act::Take(n)),
        PackSlot::Mine(at) => {
            let receives = open.is_some_and(|o| o.search.is_none());
            smart_act(inv?, at, receives, shift, ctrl, content)
        }
    }
}

/// What the container a mark's `holder` names is called.
pub(super) fn holder_name(holder: Holder, content: &Content, world: &World) -> String {
    match holder {
        Holder::Placed(k) => content
            .loot
            .kinds
            .get(usize::from(k))
            .map_or_else(|| "Container".into(), |k| k.name.clone()),
        Holder::Body(_) => "A body".into(),
        Holder::Hero(p) => world
            .hero(p)
            .map_or_else(|| "A fallen hero".into(), |h| format!("{}'s body", h.name)),
        Holder::Remains(_) => "Your remains".into(),
        Holder::Pile => "On the ground".into(),
    }
}

/// Whether the backpack is open. `NIGHTFALL_BACKPACK` opens it, for a test shot.
#[derive(Resource)]
pub(super) struct BackpackOpen(pub bool);

impl Default for BackpackOpen {
    fn default() -> Self {
        Self(std::env::var_os("NIGHTFALL_BACKPACK").is_some())
    }
}

#[derive(Component)]
pub(super) struct BackpackRoot;

#[derive(Component)]
pub(super) struct LootBody;

#[derive(Component)]
pub(super) struct PackBody;

#[derive(Component)]
pub(super) struct PackInfo;

/// The "Searching… x s" line of the loot panel, whose seconds [`rebuild`] writes in place.
#[derive(Component)]
pub(super) struct SearchLeft;

/// Which stack a slot shows: ours at a place, or the open container's in a slot.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PackSlot {
    Mine(Place),
    Loot(u16),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PackButton {
    TakeAll,
    Close,
}

#[derive(Resource)]
pub(super) struct PackFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            BackpackRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(120.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(8.0),
                ..default()
            },
            GlobalZIndex(400),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            for panel in [0, 1] {
                let node = Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                };
                let looks = (BackgroundColor(PANEL), BorderColor::all(EDGE));
                if panel == 0 {
                    root.spawn((LootBody, node, looks));
                } else {
                    root.spawn((PackBody, node, looks));
                }
            }
            root.spawn((PackInfo, text(&font, "", 11.0, GREY)));
        });
    commands.insert_resource(PackFont(font));
}

/// B opens and closes the backpack, unless a text box has the keyboard.
pub(super) fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    script: Option<NonSend<UiScript>>,
    mut open: ResMut<BackpackOpen>,
) {
    let typing = script.as_ref().is_some_and(|s| s.has_keyboard_focus());
    if keys.just_pressed(KeyCode::KeyB) && !typing {
        open.0 = !open.0;
    }
}

/// WoW's bag windows kept shut while Nightfall plays (our things are in our own panel), every
/// two seconds in case the interface was loaded again.
pub(super) fn quiet_wow_bags(
    script: Option<NonSend<UiScript>>,
    real: Res<Time<Real>>,
    mut last: Local<Option<f32>>,
) {
    let Some(script) = script else {
        return;
    };
    let now = real.elapsed_secs();
    if last.is_some_and(|t| now - t < 2.0) {
        return;
    }
    *last = Some(now);
    if let Err(e) = script.run(QUIET_BAGS) {
        warn!("nightfall: could not quiet WoW's bags: {e}");
    }
}

/// Leaving Nightfall: WoW's own bag functions back, for the other game.
pub(super) fn restore_wow_bags(script: Option<NonSend<UiScript>>) {
    if let Some(script) = script {
        if let Err(e) = script.run(RESTORE_BAGS) {
            warn!("nightfall: could not give WoW its bags back: {e}");
        }
    }
}

/// WoW's bag functions set aside once (a reloaded interface is a new Lua state, which has made
/// them afresh) and replaced by nothing.
const QUIET_BAGS: &str = "\
if not NF_SAVED_BAGS then\n\
  NF_SAVED_BAGS = { open_all = OpenAllBags, close_all = CloseAllBags, toggle_backpack = ToggleBackpack, open_backpack = OpenBackpack, toggle_bag = ToggleBag }\n\
end\n\
OpenAllBags = function() end\n\
CloseAllBags = function() end\n\
ToggleBackpack = function() end\n\
OpenBackpack = function() end\n\
ToggleBag = function() end\n";

/// [`QUIET_BAGS`] undone.
const RESTORE_BAGS: &str = "\
if NF_SAVED_BAGS then\n\
  OpenAllBags = NF_SAVED_BAGS.open_all\n\
  CloseAllBags = NF_SAVED_BAGS.close_all\n\
  ToggleBackpack = NF_SAVED_BAGS.toggle_backpack\n\
  OpenBackpack = NF_SAVED_BAGS.open_backpack\n\
  ToggleBag = NF_SAVED_BAGS.toggle_bag\n\
  NF_SAVED_BAGS = nil\n\
end\n";

/// The panels show during a run: the loot panel while a container is open (or searched), the
/// backpack while open or beside it.
pub(super) fn show(
    phase: Res<State<Phase>>,
    open: Res<BackpackOpen>,
    sim: Option<Res<Sim>>,
    mut roots: Query<&mut Visibility, With<BackpackRoot>>,
    mut loot: Query<&mut Node, (With<LootBody>, Without<PackBody>)>,
    mut pack: Query<&mut Node, (With<PackBody>, Without<LootBody>)>,
) {
    let playing = *phase.get() == Phase::Playing;
    let looting = playing
        && sim
            .as_ref()
            .is_some_and(|s| s.world.loot_view(Some(s.me)).open.is_some());
    let on = playing && (open.0 || looting);
    let want = if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
    let display = |shown: bool| {
        if shown {
            Display::Flex
        } else {
            Display::None
        }
    };
    for mut n in &mut loot {
        if n.display != display(looting) {
            n.display = display(looting);
        }
    }
    for mut n in &mut pack {
        let d = display(open.0 || looting);
        if n.display != d {
            n.display = d;
        }
    }
}

/// What the loot panel is built from: the container's name and its stacks, and whether it is
/// still being searched. The seconds a search has left are not part of it (they tick every step,
/// and are written in place, [`SearchLeft`]), so a steady search never builds the panel again.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LootKey {
    name: String,
    stacks: Vec<Option<Stack>>,
    searching: bool,
}

/// The loot panel's key for the container `view` has open, if any.
fn loot_key(view: &LootView, content: &Content, world: &World) -> Option<LootKey> {
    let open = view.open.as_ref()?;
    let holder = view
        .marks
        .iter()
        .find(|m| m.id == open.id)
        .map(|m| m.holder)
        .unwrap_or(Holder::Pile);
    Some(LootKey {
        name: holder_name(holder, content, world),
        stacks: open.stacks.clone(),
        searching: open.search.is_some(),
    })
}

/// The line a search with `left` seconds to go shows.
fn search_line(left: f32) -> String {
    format!("Searching… {left:.1} s")
}

/// The panels built again when what they show changes: the loot panel with the open container, the
/// backpack with our things, each on its own, so a change to one leaves the other alone. A search's
/// seconds are written into its text in place.
pub(super) fn rebuild(
    mut commands: Commands,
    sim: Res<Sim>,
    font: Option<Res<PackFont>>,
    loot_body: Query<Entity, With<LootBody>>,
    pack_body: Query<Entity, With<PackBody>>,
    mut searching: Query<&mut Text, With<SearchLeft>>,
    mut loot_shown: Local<Option<Option<LootKey>>>,
    mut pack_shown: Local<Option<Option<Inventory>>>,
) {
    let content = content();
    let inv = sim.world.pack(sim.me).cloned();
    let view = sim.world.loot_view(Some(sim.me));
    if let Some((left, _)) = view.open.as_ref().and_then(|o| o.search) {
        let line = search_line(left);
        for mut t in &mut searching {
            if t.0 != line {
                t.0 = line.clone();
            }
        }
    }
    let key = loot_key(&view, &content, &sim.world);
    let loot_changed = loot_shown.as_ref() != Some(&key);
    let pack_changed = pack_shown.as_ref() != Some(&inv);
    if !loot_changed && !pack_changed {
        return;
    }
    let (Ok(loot_body), Ok(pack_body), Some(font)) = (loot_body.single(), pack_body.single(), font)
    else {
        return;
    };
    let font = &font.0;
    let row = |cols: usize| Node {
        flex_wrap: FlexWrap::Wrap,
        width: Val::Px(cols as f32 * (SLOT + 3.0)),
        column_gap: Val::Px(3.0),
        row_gap: Val::Px(3.0),
        ..default()
    };
    let item_of = |s: Option<Stack>| {
        s.and_then(|s| content.items.get(usize::from(s.item)).map(|d| (d, s.count)))
    };
    if loot_changed {
        commands.entity(loot_body).despawn_related::<Children>();
        if let (Some(open), Some(key)) = (&view.open, &key) {
            commands.entity(loot_body).with_children(|b| {
                b.spawn(text(font, key.name.clone(), 13.0, GOLD));
                match open.search {
                    Some((left, _)) => {
                        b.spawn((SearchLeft, text(font, search_line(left), 12.0, GREY)));
                    }
                    None => {
                        b.spawn(row(6)).with_children(|g| {
                            for (i, s) in key.stacks.iter().enumerate() {
                                widgets::slot(g, font, PackSlot::Loot(i as u16), item_of(*s));
                            }
                        });
                        b.spawn(Node {
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|r| {
                            widgets::text_button(r, font, PackButton::TakeAll, "Take All");
                            widgets::text_button(r, font, PackButton::Close, "Close");
                        });
                    }
                }
            });
        }
        *loot_shown = Some(key);
    }
    if pack_changed {
        commands.entity(pack_body).despawn_related::<Children>();
        if let Some(inv) = &inv {
            let item_at = |at: Place| item_of(inv.get(at).copied());
            commands.entity(pack_body).with_children(|b| {
                b.spawn(text(font, "Worn", 11.0, GREY));
                b.spawn(row(EquipSlot::COUNT)).with_children(|g| {
                    for s in EquipSlot::ALL {
                        widgets::slot(
                            g,
                            font,
                            PackSlot::Mine(Place::Equip(s)),
                            item_at(Place::Equip(s)),
                        );
                    }
                });
                b.spawn(text(font, "Bags", 11.0, GREY));
                b.spawn(row(nightfall_sim::MAX_BAGS)).with_children(|g| {
                    for i in 0..nightfall_sim::MAX_BAGS as u8 {
                        widgets::slot(
                            g,
                            font,
                            PackSlot::Mine(Place::Bag(i)),
                            item_at(Place::Bag(i)),
                        );
                    }
                });
                b.spawn(text(
                    font,
                    format!("Backpack  ({} slots)", inv.pack.len()),
                    11.0,
                    GREY,
                ));
                b.spawn(row(8)).with_children(|g| {
                    for i in 0..inv.pack.len() as u16 {
                        widgets::slot(
                            g,
                            font,
                            PackSlot::Mine(Place::Pack(i)),
                            item_at(Place::Pack(i)),
                        );
                    }
                });
                b.spawn(text(font, "Safe slots: always kept", 11.0, GREY));
                b.spawn(row(inv.safe.len().max(1))).with_children(|g| {
                    for i in 0..inv.safe.len() as u8 {
                        widgets::slot(
                            g,
                            font,
                            PackSlot::Mine(Place::Safe(i)),
                            item_at(Place::Safe(i)),
                        );
                    }
                });
                b.spawn(text(
                    font,
                    "B: close  ·  Shift-click: safe slot  ·  Ctrl-click: drop",
                    10.0,
                    GREY,
                ));
            });
        }
        *pack_shown = Some(inv);
    }
}

/// Clicks into acts for the host: one act at most for a click, queued after any in flight.
pub(super) fn clicks(
    slots: Query<(&Interaction, &PackSlot), Changed<Interaction>>,
    buttons: Query<(&Interaction, &PackButton), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
) {
    let content = content();
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let view = sim.world.loot_view(Some(sim.me));
    let inv = sim.world.pack(sim.me);
    for (i, slot) in &slots {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Some(act) = click_act(*slot, inv, view.open.as_ref(), shift, ctrl, &content) {
            input.queue(act);
        }
    }
    for (i, b) in &buttons {
        // The buttons are the open container's: with none open there is nothing to take or close.
        if *i == Interaction::Pressed && view.open.is_some() {
            input.queue(match b {
                PackButton::TakeAll => Act::TakeAll,
                PackButton::Close => Act::Close,
            });
        }
    }
}

/// The line under the panels: the stack under the mouse.
pub(super) fn hover(
    slots: Query<(&Interaction, &PackSlot)>,
    sim: Res<Sim>,
    mut info: Query<&mut Text, With<PackInfo>>,
) {
    let content = content();
    let line = slots
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .and_then(|(_, s)| match *s {
            PackSlot::Mine(at) => sim.world.pack(sim.me)?.get(at).copied(),
            PackSlot::Loot(n) => sim
                .world
                .loot_view(Some(sim.me))
                .open?
                .stacks
                .get(usize::from(n))
                .copied()
                .flatten(),
        })
        .and_then(|s| {
            let def = content.items.get(usize::from(s.item))?;
            Some(widgets::describe(def, s.count))
        })
        .unwrap_or_default();
    for mut t in &mut info {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{ContentSources, Stack};

    fn content() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (9.0, 0.0), (9.0, 9.0), (0.0, 9.0)],
                entries: [(1.0, 1.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                run: (backpack_slots: 3, safe_slots: 1, conjured_drink: "water"))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand)),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 5),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 2),
                (key: "lantern", name: "Lantern", kind: Lantern, icon: "a", weight: 1.0,
                 light: Some((range: 18.0, intensity: 1.4))),
                (key: "water", name: "Water", kind: Drink, icon: "a", weight: 0.5, stack: 5),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    #[test]
    fn a_click_does_the_likely_thing() {
        let c = content();
        let mut inv = Inventory::new(&c);
        for (item, n) in [(0, 1), (1, 3), (2, 1)] {
            inv.add(Stack::new(item, n), &c);
        }
        let (knife, bread, pouch) = (Place::Pack(0), Place::Pack(1), Place::Pack(2));
        assert_eq!(
            smart_act(&inv, knife, false, false, false, &c),
            Some(Act::Move {
                from: knife,
                to: Place::Equip(EquipSlot::MainHand)
            })
        );
        assert_eq!(smart_act(&inv, bread, false, false, false, &c), None);
        assert_eq!(
            smart_act(&inv, pouch, false, false, false, &c),
            Some(Act::Move {
                from: pouch,
                to: Place::Bag(0)
            })
        );
        assert_eq!(
            smart_act(&inv, bread, true, false, false, &c),
            Some(Act::Put(bread))
        );
        assert_eq!(
            smart_act(&inv, bread, false, true, false, &c),
            Some(Act::Move {
                from: bread,
                to: Place::Safe(0)
            })
        );
        assert_eq!(
            smart_act(&inv, bread, false, false, true, &c),
            Some(Act::Drop(bread))
        );
        inv.move_to(knife, Place::Equip(EquipSlot::MainHand), &c)
            .unwrap();
        let worn = Place::Equip(EquipSlot::MainHand);
        assert_eq!(
            smart_act(&inv, worn, false, false, false, &c),
            Some(Act::Move {
                from: worn,
                to: Place::Pack(0)
            })
        );
        assert_eq!(
            smart_act(&inv, Place::Pack(0), false, false, false, &c),
            None
        );
    }

    #[test]
    fn a_lantern_is_lit_by_a_click() {
        let c = content();
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(3, 1), &c);
        assert_eq!(
            smart_act(&inv, Place::Pack(0), false, false, false, &c),
            Some(Act::Use(Place::Pack(0)))
        );
    }

    #[test]
    fn a_click_means_at_most_one_act_and_an_empty_slot_none() {
        let c = content();
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(1, 3), &c);
        let bread = Place::Pack(0);
        let searching = OpenView {
            id: 4,
            stacks: Vec::new(),
            search: Some((1.0, 2.0)),
        };
        let open = OpenView {
            id: 4,
            stacks: vec![Some(Stack::new(0, 1)), None],
            search: None,
        };
        let mine = PackSlot::Mine(bread);
        // Searching, the container takes nothing yet: bread is only bread.
        assert_eq!(
            click_act(mine, Some(&inv), Some(&searching), false, false, &c),
            None
        );
        // Searched, bread goes in.
        assert_eq!(
            click_act(mine, Some(&inv), Some(&open), false, false, &c),
            Some(Act::Put(bread))
        );
        // A stack in the container is taken; an empty slot, or no container, means nothing.
        let loot = |n| PackSlot::Loot(n);
        assert_eq!(
            click_act(loot(0), Some(&inv), Some(&open), false, false, &c),
            Some(Act::Take(0))
        );
        assert_eq!(
            click_act(loot(1), Some(&inv), Some(&open), false, false, &c),
            None
        );
        assert_eq!(click_act(loot(0), Some(&inv), None, false, false, &c), None);
        // An empty backpack slot, and no inventory at all.
        let empty = PackSlot::Mine(Place::Pack(2));
        assert_eq!(click_act(empty, Some(&inv), None, false, false, &c), None);
        assert_eq!(click_act(mine, None, None, false, false, &c), None);
    }

    /// A hero standing at the first placed container with it open and searching, in a world the
    /// panels' system reads.
    fn searching_world() -> bevy::ecs::world::World {
        use nightfall_sim::{ActPress, HeroPose, PlayerInput, RaidRoll, V2};

        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let c = super::super::shipped_content();
        let mut sim = World::new(c.clone());
        sim.begin_raid(RaidRoll::standard(&c, 1)).unwrap();
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        sim.give_loadout(
            0,
            &nightfall_sim::Loadout {
                inventory: Inventory::new(&c),
                recovery: None,
            },
        );
        let mark = sim
            .loot_view(None)
            .marks
            .into_iter()
            .find(|m| matches!(m.holder, Holder::Placed(_)))
            .expect("the raid places containers");
        let pose = Some(HeroPose {
            pos: mark.at,
            facing: V2::new(1.0, 0.0),
            z: 0.0,
        });
        let input = PlayerInput {
            pose,
            act: Some(ActPress {
                seq: 1,
                act: Act::Open(mark.id),
            }),
            ..Default::default()
        };
        for _ in 0..3 {
            sim.set_input(0, input.clone());
            sim.step();
        }
        assert!(
            sim.loot_view(Some(0))
                .open
                .is_some_and(|o| o.search.is_some()),
            "the hero is searching"
        );
        let mut app = bevy::ecs::world::World::new();
        app.insert_resource(Sim::for_test(sim));
        app.insert_resource(PackFont(Handle::default()));
        app.spawn((LootBody, Node::default()));
        app.spawn((PackBody, Node::default()));
        app
    }

    fn panel_children(app: &mut bevy::ecs::world::World) -> (Vec<Entity>, Vec<Entity>) {
        let kids = |app: &mut bevy::ecs::world::World, of: Entity| {
            app.get::<Children>(of)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let loot = app
            .query_filtered::<Entity, With<LootBody>>()
            .single(app)
            .unwrap();
        let pack = app
            .query_filtered::<Entity, With<PackBody>>()
            .single(app)
            .unwrap();
        (kids(app, loot), kids(app, pack))
    }

    #[test]
    fn a_steady_search_writes_its_seconds_in_place_and_builds_nothing_again() {
        let mut app = searching_world();
        let rebuild = app.register_system(rebuild);
        app.run_system(rebuild).unwrap();
        let (loot, pack) = panel_children(&mut app);
        assert!(
            !loot.is_empty() && !pack.is_empty(),
            "both panels were built"
        );
        let said = |app: &mut bevy::ecs::world::World| {
            app.query_filtered::<&Text, With<SearchLeft>>()
                .single(app)
                .unwrap()
                .0
                .clone()
        };
        let before = said(&mut app);
        assert!(before.starts_with("Searching"), "{before}");

        // The search goes on: seconds tick, the panels' children are the very same entities.
        for _ in 0..6 {
            app.resource_mut::<Sim>().world.step();
        }
        app.run_system(rebuild).unwrap();
        assert_eq!(panel_children(&mut app), (loot, pack));
        assert_ne!(said(&mut app), before, "the seconds were written in place");
    }
}
