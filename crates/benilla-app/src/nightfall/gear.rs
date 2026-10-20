//! What heroes carry, seen: the gear every hero wears (stand-in item templates, as
//! [`super::dress`] files the starting outfits), a lit lantern's light, and our load under the
//! player frame.

use std::collections::HashMap;
use std::f32::consts::PI;

use benilla_protocol::messages::ObjectFields;
use benilla_world::lighting::WorldPointLight;
use bevy::prelude::*;
use nightfall_sim::{Content, EquipSlot, ItemDef, Look, PlayerId};

use super::dress::{self, Piece};
use super::run::Sim;
use super::view::Views;
use super::widgets::{text, GREY, RED};
use super::{content, Phase};
use crate::net::ObjectStore;

/// The piece `def` shows worn in WoW equipment slot `wow`; `outfit_of(class_id)` is the starting
/// outfit a `Look::Outfit` borrows from, in the hero's race and sex.
pub(super) fn piece_of(
    def: &ItemDef,
    wow: u8,
    outfit_of: impl Fn(u8) -> Vec<Piece>,
) -> Option<Piece> {
    match def.look? {
        Look::Item {
            display,
            inventory_type,
            class,
            subclass,
            material,
            sheath,
        } => Some(Piece {
            slot: wow,
            display,
            inventory_type: u32::from(inventory_type),
            class: u32::from(class),
            subclass: u32::from(subclass),
            material: u32::from(material),
            sheath: u32::from(sheath),
        }),
        Look::Outfit { class_id } => outfit_of(class_id).into_iter().find(|p| p.slot == wow),
    }
}

/// What a hero wearing `worn` shows in each of our equipment slots: the worn piece, else the
/// starting outfit's (`base`), else nothing. The slots that show nothing come back apart, to be
/// written 0, which takes a class weapon out of an empty hand.
pub(super) fn pieces_worn(
    worn: &[(EquipSlot, u16)],
    content: &Content,
    base: &[Piece],
    outfit_of: impl Fn(u8) -> Vec<Piece>,
) -> (Vec<Piece>, Vec<u8>) {
    let mut pieces = Vec::new();
    let mut empty = Vec::new();
    for slot in EquipSlot::ALL {
        let wow = slot.wow_slot();
        let worn_piece = worn
            .iter()
            .find(|(s, _)| *s == slot)
            .and_then(|&(_, i)| content.items.get(usize::from(i)))
            .and_then(|d| piece_of(d, wow, &outfit_of));
        match worn_piece.or_else(|| base.iter().find(|p| p.slot == wow).cloned()) {
            Some(p) => pieces.push(p),
            None => empty.push(wow),
        }
    }
    (pieces, empty)
}

/// What [`wear`] last wrote for a hero: the body it dressed and the gear it dressed it in.
type Dressed = (Entity, Vec<(EquipSlot, u16)>);

/// Whether a hero's body must be dressed again: it has not been dressed, it is another body (a new
/// run's, for the same player), or what it wears changed.
fn needs_dressing(shown: Option<&Dressed>, body: Entity, worn: &[(EquipSlot, u16)]) -> bool {
    !shown.is_some_and(|(e, w)| *e == body && w == worn)
}

/// Every hero dressed in what they wear, written again only when it (or the body) changes. The
/// `Sim` is written every frame, so Bevy's own change detection cannot tell: the last gear written
/// is what each frame is compared with.
pub(super) fn wear(
    sim: Res<Sim>,
    views: Res<Views>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut stores: Query<&mut ObjectStore>,
    mut shown: Local<HashMap<PlayerId, Dressed>>,
) {
    let Some(cc) = char_create.as_deref() else {
        return;
    };
    let content = content();
    shown.retain(|p, _| views.heroes.contains_key(p));
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        if !needs_dressing(shown.get(&hero.player), e, &hero.carry.worn) {
            continue;
        }
        let Ok(mut store) = stores.get_mut(e) else {
            continue;
        };
        let class = &content.classes[hero.class];
        let outfit_of =
            |class_id: u8| dress::pieces(cc.0.start_outfit(hero.race, class_id, hero.sex), &[]);
        let base = outfit_of(class.class_id);
        let (pieces, empty) = pieces_worn(&hero.carry.worn, &content, &base, outfit_of);
        let mut pairs = dress::wear(&mut items, &pieces);
        pairs.extend(
            empty
                .into_iter()
                .map(|s| (dress::visible_entry_field(s), 0)),
        );
        store.0.merge(ObjectFields::from_pairs(&pairs));
        shown.insert(hero.player, (e, hero.carry.worn.clone()));
    }
}

/// A lit lantern's light around its hero (spec §10), the small glow otherwise.
pub(super) fn lanterns(
    sim: Res<Sim>,
    views: Res<Views>,
    children: Query<&Children>,
    mut lights: Query<&mut WorldPointLight>,
) {
    let content = content();
    let base = &content.rules.atmosphere.hero_light;
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let (range, intensity) = match hero
            .carry
            .lantern
            .and_then(|i| content.items.get(usize::from(i)))
            .and_then(|d| d.light)
        {
            Some(l) => (l.range, l.intensity),
            None => (base.range, base.intensity),
        };
        // `4π × intensity`, the point-light convention the packer undoes.
        let watts = 4.0 * PI * intensity;
        let Ok(kids) = children.get(e) else {
            continue;
        };
        for k in kids.iter() {
            if let Ok(mut l) = lights.get_mut(k) {
                if l.range != range || l.intensity != watts {
                    l.range = range;
                    l.intensity = watts;
                }
            }
        }
    }
}

/// Our load, under the player frame.
#[derive(Component)]
pub(super) struct LoadLine;

/// Spawns the load line once at startup, hidden: [`load_line`] writes and shows it.
pub(super) fn spawn_line(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        LoadLine,
        text(&font, "", 12.0, GREY),
        Node {
            position_type: PositionType::Absolute,
            // Right of the dodge ring, which sits under the player frame at the left edge.
            left: Val::Px(92.0),
            top: Val::Px(100.0),
            ..default()
        },
        GlobalZIndex(350),
        Visibility::Hidden,
    ));
}

/// "Load 34.0: 89% speed, louder steps", red once it slows us; the lantern, when lit.
pub(super) fn load_line(
    sim: Option<Res<Sim>>,
    phase: Res<State<Phase>>,
    mut lines: Query<(&mut Text, &mut TextColor, &mut Visibility), With<LoadLine>>,
) {
    let hero = sim
        .as_ref()
        .filter(|_| *phase.get() == Phase::Playing)
        .and_then(|s| s.world.hero(s.me));
    let Some(h) = hero else {
        for (_, _, mut v) in &mut lines {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        return;
    };
    let content = content();
    for (mut t, mut c, mut v) in &mut lines {
        let r = &content.rules.run;
        let (pace, _) = nightfall_sim::burden(h.carry.load, r);
        let mut line = if pace < 1.0 {
            format!(
                "Load {:.1}: {:.0}% speed, louder steps",
                h.carry.load,
                pace * 100.0
            )
        } else {
            format!("Load {:.1} of {:.0}", h.carry.load, r.weight_free)
        };
        if h.carry.lantern.is_some() {
            line.push_str("  ·  lantern lit");
        }
        let color = if pace < 1.0 { RED } else { GREY };
        if t.0 != line {
            t.0 = line;
        }
        if c.0 != color {
            c.0 = color;
        }
        if *v != Visibility::Inherited {
            *v = Visibility::Inherited;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::ContentSources;

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
                run: (conjured_drink: "water"))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "blade", name: "Blade", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand),
                 look: Some(Item(display: 7492, inventory_type: 13, class: 2, subclass: 7,
                                 material: 1, sheath: 3))),
                (key: "tunic", name: "Tunic", kind: Armour, icon: "a", weight: 1.0,
                 slot: Some(Chest), look: Some(Outfit(class_id: 1))),
                (key: "water", name: "Water", kind: Drink, icon: "a", weight: 0.5, stack: 5),
            ]"#,
            loot: "()",
            monsters: "[]",
            rivals: "",
        })
        .unwrap()
    }

    fn cloth(slot: u8, display: u32) -> Piece {
        Piece {
            slot,
            display,
            inventory_type: 5,
            class: 4,
            subclass: 1,
            material: 7,
            sheath: 0,
        }
    }

    #[test]
    fn worn_gear_covers_the_outfit_and_an_empty_hand_shows_nothing() {
        let c = content();
        let base = vec![cloth(4, 100), cloth(6, 101)];
        let warrior = |class_id: u8| {
            if class_id == 1 {
                vec![cloth(4, 900)]
            } else {
                Vec::new()
            }
        };
        let (pieces, empty) = pieces_worn(
            &[(EquipSlot::MainHand, 0), (EquipSlot::Chest, 1)],
            &c,
            &base,
            warrior,
        );
        assert!(
            pieces.contains(&cloth(4, 900)),
            "the tunic borrows the warrior's chest"
        );
        assert!(
            pieces.contains(&cloth(6, 101)),
            "the legs stay the starting outfit's"
        );
        assert!(pieces.iter().any(|p| p.slot == 15 && p.display == 7492));
        assert!(empty.contains(&16), "nothing in the off hand");
        assert!(!empty.contains(&15));
        let (_, bare) = pieces_worn(&[], &c, &base, |_| Vec::new());
        assert!(bare.contains(&15) && bare.contains(&16));
    }

    #[test]
    fn a_body_is_dressed_once_per_change_of_gear_or_body() {
        let mut bodies = World::new();
        let (a, b) = (bodies.spawn_empty().id(), bodies.spawn_empty().id());
        let blade = vec![(EquipSlot::MainHand, 0)];
        // Never dressed.
        assert!(needs_dressing(None, a, &blade));
        let shown = (a, blade.clone());
        // The same gear on the same body: nothing to write.
        assert!(!needs_dressing(Some(&shown), a, &blade));
        // The gear changed.
        assert!(needs_dressing(Some(&shown), a, &[]));
        // Another body for the same player (a new run) with the very same gear is still bare.
        assert!(needs_dressing(Some(&shown), b, &blade));
    }

    #[test]
    fn a_lit_lantern_lights_its_hero_and_goes_out_with_it() {
        use bevy::ecs::system::RunSystemOnce;

        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let c = super::super::shipped_content();
        let lantern = c.item("hooded_lantern").unwrap();
        let wanted = c.items[lantern].light.expect("a lantern gives light");
        let base = &c.rules.atmosphere.hero_light;
        let mut sim = nightfall_sim::World::new(c.clone());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        let mut world = World::new();
        let body = world.spawn_empty().id();
        // The small glow the hero has from the start (`atmosphere::hero_lights`).
        let glow = world
            .spawn(WorldPointLight {
                color: [1.0, 0.8, 0.6],
                intensity: 4.0 * PI * base.intensity,
                range: base.range,
            })
            .id();
        world.entity_mut(body).add_child(glow);
        let mut views = Views::default();
        views.heroes.insert(0, body);
        world.insert_resource(views);
        world.insert_resource(Sim::for_test(sim));
        let light = |world: &World| {
            let l = world.get::<WorldPointLight>(glow).unwrap();
            (l.range, l.intensity)
        };

        assert_eq!(light(&world), (base.range, 4.0 * PI * base.intensity));
        world.resource_mut::<Sim>().world.heroes[0].carry.lantern = Some(lantern as u16);
        world.run_system_once(lanterns).unwrap();
        assert_eq!(light(&world), (wanted.range, 4.0 * PI * wanted.intensity));
        // Put out: the small glow again.
        world.resource_mut::<Sim>().world.heroes[0].carry.lantern = None;
        world.run_system_once(lanterns).unwrap();
        assert_eq!(light(&world), (base.range, 4.0 * PI * base.intensity));
    }
}
