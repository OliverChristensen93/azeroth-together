//! What meta equipment looks like on a hero, and its icon.
//!
//! A piece's look follows from its slot, armour, rarity and seed, so it looks the same on every
//! screen and for every player: common and uncommon pieces wear plain armour of their type, rare
//! ones a dungeon set (WoW's tier 0), epic ones the first or second raid tier, and the uniques
//! their own famous looks (Thunderfury's blade, Atiesh's staff, tier 3 Frostfire for the
//! sandals). The set of a piece's armour type is picked by its seed, so cloth may be a mage's,
//! a priest's or a warlock's set. Every display id below has its model and textures in the
//! game's trimmed data; the data has no cape textures, so a back piece changes nothing to see.
//!
//! The hero is dressed like the menu's preview and the run's body ([`super::dress`]): each
//! piece becomes a stand-in item template, and its entry goes into the player's
//! `PLAYER_VISIBLE_ITEM_*` field. A run's hero shows only the pieces switched on, so it gears up
//! as gold is spent; the menu's hero shows everything it will bring.

use std::collections::HashMap;

use bevy::prelude::*;
use together_sim::{Armor, GearPiece, PlayerId, Rarity, Slot, Unique, WeaponKind};

use super::dress::{self, Piece};
use super::{content, Sim};
use crate::net::ObjectStore;

/// `PLAYER_VISIBLE_ITEM_1_CREATOR`: each equipment slot has 12 fields, the entry third.
const FIELD_VISIBLE_ITEM_BASE: u16 = 258;
const VISIBLE_ITEM_STRIDE: u16 = 12;

/// The field that holds the item entry worn in equipment slot `slot`.
fn entry_field(slot: u8) -> u16 {
    FIELD_VISIBLE_ITEM_BASE + 2 + VISIBLE_ITEM_STRIDE * u16::from(slot)
}

/// The player equipment slot (`EQUIPMENT_SLOT_*`) a gear slot shows in.
fn equipment_slot(slot: Slot) -> u8 {
    match slot {
        Slot::Head => 0,
        Slot::Shoulders => 2,
        Slot::Chest => 4,
        Slot::Waist => 5,
        Slot::Legs => 6,
        Slot::Feet => 7,
        Slot::Wrists => 8,
        Slot::Hands => 9,
        Slot::Back => 14,
        Slot::MainHand => 15,
        Slot::OffHand => 16,
    }
}

/// An armour set's displays: head, shoulders, chest, wrists, hands, waist, legs and feet; 0
/// where the data lacks the piece.
struct Set {
    armor: Armor,
    /// 0 the dungeon set, 1 and 2 the first two raid tiers, 3 the third.
    tier: u8,
    pieces: [u32; 8],
}

const fn set(armor: Armor, tier: u8, pieces: [u32; 8]) -> Set {
    Set {
        armor,
        tier,
        pieces,
    }
}

/// Every class set in the data, by armour: the mage's, priest's and warlock's in cloth, the
/// rogue's and druid's in leather, the hunter's and shaman's in mail, the warrior's and
/// paladin's in plate.
const SETS: &[Set] = &[
    // Magister's, Devout and Dreadmist (dungeon).
    set(
        Armor::Cloth,
        0,
        [0, 30211, 29591, 29595, 29593, 29596, 34619, 29594],
    ),
    set(
        Armor::Cloth,
        0,
        [0, 30428, 30422, 0, 30427, 30425, 30423, 30429],
    ),
    set(
        Armor::Cloth,
        0,
        [31261, 0, 29792, 29795, 29800, 29793, 29797, 29799],
    ),
    // Arcanist, Prophecy and Felheart.
    set(
        Armor::Cloth,
        1,
        [31512, 30586, 30581, 0, 30585, 30583, 30582, 30587],
    ),
    set(
        Armor::Cloth,
        1,
        [31206, 30623, 31490, 30617, 30618, 30621, 0, 30622],
    ),
    set(
        Armor::Cloth,
        1,
        [31987, 31974, 31973, 31970, 31971, 31969, 31972, 31975],
    ),
    // Netherwind, Transcendence and Nemesis.
    set(
        Armor::Cloth,
        2,
        [34057, 34058, 34038, 34045, 34041, 34046, 34039, 34043],
    ),
    set(
        Armor::Cloth,
        2,
        [34054, 34048, 34047, 34052, 34051, 34053, 34049, 34055],
    ),
    set(
        Armor::Cloth,
        2,
        [33644, 34022, 34014, 34012, 34013, 34011, 0, 34010],
    ),
    // Frostfire, Faith and Plagueheart.
    set(
        Armor::Cloth,
        3,
        [0, 35326, 35523, 0, 35521, 35519, 35522, 35524],
    ),
    set(
        Armor::Cloth,
        3,
        [0, 35149, 35142, 35144, 35145, 35143, 35153, 35148],
    ),
    set(
        Armor::Cloth,
        3,
        [0, 35187, 35185, 35180, 35183, 35179, 35184, 35186],
    ),
    // Wildheart (dungeon); Nightslayer and Cenarion; Bloodfang and Stormrage; Bonescythe and
    // Dreamwalker.
    set(
        Armor::Leather,
        0,
        [31228, 30412, 29974, 29977, 29978, 29976, 29975, 0],
    ),
    set(
        Armor::Leather,
        1,
        [31501, 31343, 31105, 31106, 31107, 31339, 31340, 31109],
    ),
    set(
        Armor::Leather,
        1,
        [31727, 32016, 31731, 31725, 31726, 31722, 31729, 31723],
    ),
    set(
        Armor::Leather,
        2,
        [33743, 32043, 31113, 31112, 31114, 31110, 31108, 31111],
    ),
    set(
        Armor::Leather,
        2,
        [33655, 30546, 30536, 30543, 30545, 30541, 30538, 30542],
    ),
    set(
        Armor::Leather,
        3,
        [35061, 35064, 35054, 35053, 35055, 36349, 35065, 35062],
    ),
    set(
        Armor::Leather,
        3,
        [35162, 35160, 35159, 35158, 35163, 35164, 35161, 35165],
    ),
    // Beaststalker and The Elements; Giantstalker and Earthfury; Dragonstalker and Ten Storms;
    // Cryptstalker and Earthshatter.
    set(
        Armor::Mail,
        0,
        [31410, 31409, 31402, 31405, 31406, 31404, 31403, 31407],
    ),
    set(
        Armor::Mail,
        0,
        [31117, 30925, 31416, 31411, 31414, 31413, 31415, 0],
    ),
    set(
        Armor::Mail,
        1,
        [32017, 32030, 32022, 32018, 32024, 32019, 32023, 32020],
    ),
    set(
        Armor::Mail,
        1,
        [31835, 31833, 31832, 31831, 31834, 31829, 31836, 31830],
    ),
    set(
        Armor::Mail,
        2,
        [34085, 34091, 33667, 33666, 33668, 33665, 33672, 33669],
    ),
    set(
        Armor::Mail,
        2,
        [34066, 34067, 34080, 34079, 34082, 34078, 34084, 34083],
    ),
    set(
        Armor::Mail,
        3,
        [35601, 35611, 35415, 35416, 35411, 35410, 35413, 35409],
    ),
    set(
        Armor::Mail,
        3,
        [35749, 35751, 35752, 35753, 35748, 35747, 35754, 35746],
    ),
    // Valor and Lightforge; Might and Lawbringer; Wrath and Judgement; Dreadnaught and
    // Redemption.
    set(
        Armor::Plate,
        0,
        [31284, 29964, 29958, 29961, 29962, 29959, 29963, 29960],
    ),
    set(
        Armor::Plate,
        0,
        [31089, 29971, 29969, 29968, 29970, 29966, 29972, 29967],
    ),
    set(
        Armor::Plate,
        1,
        [31259, 31024, 31021, 31020, 31022, 31019, 31023, 31025],
    ),
    set(
        Armor::Plate,
        1,
        [31506, 31510, 31505, 31509, 31507, 31353, 31352, 31354],
    ),
    set(
        Armor::Plate,
        2,
        [33741, 33679, 33628, 33627, 33629, 33632, 33630, 33631],
    ),
    set(
        Armor::Plate,
        2,
        [33682, 33680, 33635, 33634, 33636, 33633, 33637, 33638],
    ),
    set(
        Armor::Plate,
        3,
        [35060, 35176, 35049, 35044, 35050, 35058, 35051, 35052],
    ),
    set(
        Armor::Plate,
        3,
        [0, 35617, 35618, 35619, 35615, 35614, 35616, 35613],
    ),
];

/// Plain armour for common and uncommon pieces: two looks per piece, by armour type, in the same
/// head-to-feet order as [`SETS`].
fn plain(armor: Armor) -> [[u32; 2]; 8] {
    match armor {
        Armor::Cloth => [
            [3171, 3960],
            [5093, 5115],
            [14388, 12377],
            [4094, 4068],
            [2164, 11041],
            [26019, 4634],
            [1882, 4619],
            [2198, 3261],
        ],
        Armor::Leather => [
            [1124, 3785],
            [1057, 4486],
            [2106, 2019],
            [3597, 3605],
            [4470, 7823],
            [3247, 3252],
            [2176, 4469],
            [2094, 3066],
        ],
        Armor::Mail => [
            [1131, 27772],
            [4109, 5000],
            [954, 3082],
            [3622, 3739],
            [1407, 3085],
            [12298, 34594],
            [685, 3083],
            [2229, 34947],
        ],
        Armor::Plate => [
            [4415, 9421],
            [4890, 15890],
            [4151, 5242],
            [4448, 25829],
            [3773, 3983],
            [3254, 3255],
            [3414, 16092],
            [16112, 16113],
        ],
    }
}

/// Capes, for their icons: the data has no cape textures, so none shows on the back.
const CAPES: [u32; 4] = [7892, 9943, 9978, 18840];

/// A weapon kind's displays at a quality step: plain (0), better (1), fine (2), epic (3).
fn weapons(kind: WeaponKind, step: usize) -> &'static [u32] {
    let table: [&[u32]; 4] = match kind {
        WeaponKind::Staff => [
            &[472, 1201, 1376],
            &[1193, 1648, 5546],
            &[1373, 2509, 2841],
            &[1190, 15990, 20256, 33420],
        ],
        WeaponKind::Greatsword => [
            &[563, 1621, 2683],
            &[1638, 2406, 3758],
            &[729, 1207, 20616],
            &[1639, 22905, 23385],
        ],
        WeaponKind::Sword => [
            &[679, 1550, 1556],
            &[862, 1592, 1609],
            &[1928, 4090, 20030],
            &[2806, 3447, 33839],
        ],
        WeaponKind::Mace => [
            &[1160, 1475, 1774],
            &[1402, 1517, 2430],
            &[21342, 1784],
            &[1022, 32533, 33731],
        ],
        WeaponKind::Axe => [
            &[453, 1384, 1386],
            &[1389, 1390, 4706],
            &[1409, 2807, 4910],
            &[1932, 31611, 32769],
        ],
        WeaponKind::Dagger => [
            &[1845, 2699, 2669],
            &[2707, 3175, 3363],
            &[2711, 2737, 32541],
            &[32527, 33430, 33734],
        ],
        WeaponKind::Shield => [
            &[1705, 2052, 2181],
            &[2080, 15883, 18456],
            &[23825, 23835, 20987],
            &[32559, 33867, 31852],
        ],
        WeaponKind::Orb => [
            &[23171, 23177],
            &[2747, 23322, 23324],
            &[5926, 8043, 15884, 36265],
            &[15725, 32767, 36268],
        ],
    };
    table[step.min(3)]
}

/// What a stand-in item template needs to show a piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Look {
    pub display: u32,
    pub inventory_type: u32,
    pub class: u32,
    pub subclass: u32,
    pub material: u32,
    pub sheath: u32,
}

/// A unique's own display.
fn unique_display(u: Unique) -> u32 {
    match u {
        // Thunderfury's blade, as the 1.12 item has it.
        Unique::Thunderfury => 30606,
        // Atiesh, the mage's.
        Unique::Atiesh => 35631,
        // Netherwind Robes.
        Unique::RobeOfTheArchmage => 34038,
        // Nemesis Spaulders.
        Unique::MantleOfTheBlackwingCabal => 34022,
        Unique::GlacialCloak => 9978,
        // Frostfire Sandals, from the mage's tier 3.
        Unique::FrostfireSandals => 35524,
        // Arcanist Gloves.
        Unique::GlovesOfTheHypnoticFlame => 30585,
        // Ashkandi's blade.
        Unique::Ashkandi => 31821,
        // Nemesis Skullcap itself.
        Unique::NemesisSkullcap => 33644,
    }
}

/// `n` mixed from a piece's seed, for picking among looks.
fn pick(seed: u32, salt: u32, n: usize) -> usize {
    let mixed = seed.wrapping_mul(2_654_435_761).rotate_left(salt) ^ salt.wrapping_mul(97);
    mixed as usize % n.max(1)
}

/// How a piece looks.
pub(super) fn look(piece: &GearPiece) -> Look {
    let display = display(piece);
    match piece.weapon() {
        Some(w) => {
            // (class, subclass, material, inventory type, sheath)
            let (class, subclass, material, inventory_type, sheath) = match w {
                WeaponKind::Staff => (2, 10, 2, 17, 2),
                WeaponKind::Greatsword => (2, 8, 1, 17, 1),
                WeaponKind::Sword => (2, 7, 1, 13, 3),
                WeaponKind::Mace => (2, 4, 1, 13, 3),
                WeaponKind::Axe => (2, 0, 1, 13, 3),
                WeaponKind::Dagger => (2, 15, 1, 13, 3),
                WeaponKind::Shield => (4, 6, 1, 14, 4),
                WeaponKind::Orb => (4, 0, 4, 23, 0),
            };
            Look {
                display,
                inventory_type,
                class,
                subclass,
                material,
                sheath,
            }
        }
        None => {
            let (subclass, material) = match piece.armor {
                Armor::Cloth => (1, 7),
                Armor::Leather => (2, 8),
                Armor::Mail => (3, 5),
                Armor::Plate => (4, 6),
            };
            let inventory_type = match piece.slot {
                Slot::Head => 1,
                Slot::Shoulders => 3,
                Slot::Back => 16,
                Slot::Chest if piece.armor == Armor::Cloth => 20,
                Slot::Chest => 5,
                Slot::Wrists => 9,
                Slot::Hands => 10,
                Slot::Waist => 6,
                Slot::Legs => 7,
                Slot::Feet => 8,
                Slot::MainHand | Slot::OffHand => 13,
            };
            Look {
                display,
                inventory_type,
                class: 4,
                subclass: if piece.slot == Slot::Back {
                    1
                } else {
                    subclass
                },
                material: if piece.slot == Slot::Back {
                    7
                } else {
                    material
                },
                sheath: 0,
            }
        }
    }
}

/// The `ItemDisplayInfo` row a piece shows.
pub(super) fn display(piece: &GearPiece) -> u32 {
    if let Some(u) = piece.unique {
        return unique_display(u);
    }
    if let Some(w) = piece.weapon() {
        let step = match piece.rarity {
            Rarity::Common => 0,
            Rarity::Uncommon => 1,
            Rarity::Rare => 2,
            Rarity::Epic | Rarity::Legendary => 3,
        };
        let list = weapons(w, step);
        return list[pick(piece.seed, 3, list.len())];
    }
    let Some(i) = armour_index(piece.slot) else {
        return CAPES[pick(piece.seed, 5, CAPES.len())];
    };
    let plain_look = || plain(piece.armor)[i][pick(piece.seed, 7, 2)];
    let tier = match piece.rarity {
        Rarity::Common | Rarity::Uncommon => return plain_look(),
        Rarity::Rare => 0,
        Rarity::Epic => 1 + pick(piece.seed, 11, 2) as u8,
        Rarity::Legendary => 3,
    };
    let sets: Vec<&Set> = SETS
        .iter()
        .filter(|s| s.armor == piece.armor && s.tier == tier && s.pieces[i] != 0)
        .collect();
    if sets.is_empty() {
        return plain_look();
    }
    // The same set for every piece of one seed's pick, so a lucky run can match.
    sets[pick(piece.seed, 13, sets.len())].pieces[i]
}

/// A body slot's place in [`SETS`]' head-to-feet order; `None` for the back and the hands'
/// weapons.
fn armour_index(slot: Slot) -> Option<usize> {
    Some(match slot {
        Slot::Head => 0,
        Slot::Shoulders => 1,
        Slot::Chest => 2,
        Slot::Wrists => 3,
        Slot::Hands => 4,
        Slot::Waist => 5,
        Slot::Legs => 6,
        Slot::Feet => 7,
        Slot::Back | Slot::MainHand | Slot::OffHand => return None,
    })
}

/// A piece as [`dress::wear`] takes it.
fn worn_piece(piece: &GearPiece) -> Piece {
    let l = look(piece);
    Piece {
        slot: equipment_slot(piece.slot),
        display: l.display,
        inventory_type: l.inventory_type,
        class: l.class,
        subclass: l.subclass,
        material: l.material,
        sheath: l.sheath,
    }
}

/// Every visible-item field of a hero of `class`, `race` and `sex` in its starting outfit and
/// weapons with `gear` over them: a piece replaces the outfit's in its slot, a two-handed weapon
/// empties the off hand, and every slot gear can fill that ends up empty is cleared (so taking
/// a piece off at the menu shows the outfit's again).
pub(super) fn outfit_fields(
    items: &mut crate::items::Items,
    char_create: &crate::entities::CharCreate,
    class: &together_sim::ClassDef,
    (race, sex): (u8, u8),
    gear: &[GearPiece],
) -> Vec<(u16, u32)> {
    let race = dress::drawn_race(char_create, race, sex);
    let outfit = char_create.0.start_outfit(race, class.class_id, sex);
    let mut pieces = dress::pieces(outfit, &class.weapons);
    let two_hand = gear.iter().any(GearPiece::two_hand);
    pieces.retain(|p| {
        !gear.iter().any(|g| equipment_slot(g.slot) == p.slot)
            && !(two_hand && p.slot == equipment_slot(Slot::OffHand))
    });
    pieces.extend(gear.iter().map(worn_piece));
    let mut fields = dress::wear(items, &pieces);
    for slot in Slot::ALL {
        let field = entry_field(equipment_slot(slot));
        if !fields.iter().any(|&(f, _)| f == field) {
            fields.push((field, 0));
        }
    }
    fields
}

/// The run's heroes wear the pieces they have switched on: dressed anew whenever one goes on.
#[allow(clippy::type_complexity)]
pub(super) fn dress_heroes(
    sim: Res<Sim>,
    views: Res<super::view::Views>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut stores: Query<&mut ObjectStore>,
    mut shown: Local<HashMap<PlayerId, (Entity, Vec<GearPiece>)>>,
) {
    let Some(char_create) = char_create.as_deref() else {
        return;
    };
    let content = content();
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let active: Vec<GearPiece> = hero.gear.active().copied().collect();
        let was = shown.get(&hero.player);
        let unchanged = match was {
            Some((we, wg)) => *we == e && *wg == active,
            // A body just spawned already wears its outfit.
            None => active.is_empty(),
        };
        if unchanged {
            continue;
        }
        let Ok(mut store) = stores.get_mut(e) else {
            continue;
        };
        let class = &content.classes[hero.class];
        let fields = outfit_fields(
            &mut items,
            char_create,
            class,
            (hero.race, hero.sex),
            &active,
        );
        store
            .0
            .merge(benilla_protocol::messages::ObjectFields::from_pairs(
                &fields,
            ));
        shown.insert(hero.player, (e, active));
    }
    shown.retain(|p, _| sim.world.hero(*p).is_some());
}

/// The menu's hero wears everything its class will bring, so the stash's choices show on it.
#[allow(clippy::type_complexity)]
pub(super) fn dress_preview(
    selection: Res<super::menu::Selection>,
    store: Res<super::armory::GearStore>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut heroes: Query<(Entity, &mut ObjectStore), With<super::menu::PreviewHero>>,
    mut shown: Local<Option<(Entity, Vec<GearPiece>)>>,
) {
    let (Some(char_create), Some((_, race, sex))) = (char_create.as_deref(), selection.hero_look())
    else {
        return;
    };
    let Ok((e, mut object)) = heroes.single_mut() else {
        *shown = None;
        return;
    };
    let content = content();
    let class = &content.classes[selection.class];
    let gear = store.armory.equipped(&class.key);
    if shown
        .as_ref()
        .is_some_and(|(se, sg)| *se == e && *sg == gear)
    {
        return;
    }
    if shown.is_none() && gear.is_empty() {
        *shown = Some((e, gear));
        return;
    }
    let fields = outfit_fields(&mut items, char_create, class, (race, sex), &gear);
    object
        .0
        .merge(benilla_protocol::messages::ObjectFields::from_pairs(
            &fields,
        ));
    *shown = Some((e, gear));
}

/// A gear icon slot: filled from the piece's display's own icon once the data is up.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct GearIcon(pub u32);

#[derive(Component)]
pub(super) struct GearIconDone;

/// Fill [`GearIcon`] slots once the item displays and the asset reader are up.
pub(super) fn fill_gear_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &GearIcon), Without<GearIconDone>>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(displays), Some(assets)) = (displays, assets.as_deref_mut()) else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let path = displays
            .catalog
            .get(icon.0)
            .and_then(|d| d.icon.clone())
            .map(|i| format!("{i}.blp"));
        if let Some(h) = path.and_then(|p| assets.sprite_texture(&p, &mut images)) {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(GearIconDone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_has_a_look_and_rarer_ones_wear_sets() {
        for armor in Armor::ALL {
            for slot in Slot::ALL {
                for rarity in [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic] {
                    for seed in 0..40 {
                        let p = GearPiece::new(slot, rarity, 30, armor, seed);
                        assert_ne!(display(&p), 0, "{p:?}");
                    }
                }
            }
        }
        for u in Unique::ALL {
            let p = GearPiece::unique(u, 30, 1);
            assert_eq!(display(&p), unique_display(u));
        }
        // A rare chest is a dungeon set's.
        let p = GearPiece::new(Slot::Chest, Rarity::Rare, 30, Armor::Plate, 5);
        assert!(SETS
            .iter()
            .any(|s| s.tier == 0 && s.pieces.contains(&display(&p))));
    }

    #[test]
    fn a_staff_is_two_handed_and_an_orb_is_held() {
        let staff = GearPiece::unique(Unique::Atiesh, 30, 1);
        assert_eq!(look(&staff).inventory_type, 17);
        let orb = GearPiece::new(Slot::OffHand, Rarity::Rare, 30, Armor::Cloth, 1);
        assert_eq!(look(&orb).inventory_type, 23);
        assert_eq!(equipment_slot(Slot::OffHand), 16);
        assert_eq!(entry_field(15), 258 + 2 + 12 * 15);
    }
}
