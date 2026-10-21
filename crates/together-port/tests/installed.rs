//! The installed port, read through the client's own loaders: `WoW/Data/patch-W.MPQ` over the
//! 1.12 archives. Skipped where the port is not installed.

use std::io::Cursor;
use std::path::PathBuf;

use benilla_formats::{CharCreateCatalog, Chain};

fn data() -> Option<PathBuf> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../WoW/Data");
    data.join("patch-W.MPQ").exists().then_some(data)
}

#[test]
fn ported_races_are_creatable_with_bodies_dials_and_outfits() {
    let Some(data) = data() else {
        eprintln!("skipped: no WoW/Data/patch-W.MPQ");
        return;
    };
    let mut chain = Chain::open(&data).expect("open chain");
    let cat = CharCreateCatalog::load(&mut chain).expect("load the create catalog");
    for (race, file) in [(10u8, "BloodElf"), (11, "Draenei")] {
        assert_eq!(cat.race_file(race), Some(file));
        let classes = cat.classes_for_race(race);
        assert!(!classes.is_empty() && !classes.contains(&6), "race {race}: classes {classes:?}");
        for sex in 0..2u8 {
            let display = cat.body_display(race, sex).unwrap_or_else(|| panic!("race {race} sex {sex}: no body"));
            let ranges = cat.ranges(race, sex).unwrap_or_else(|| panic!("race {race} sex {sex}: no dials"));
            eprintln!("race {race} sex {sex}: display {display}, dials {ranges:?}");
            for &class in &classes {
                assert!(!cat.start_outfit(race, class, sex).is_empty(), "race {race} class {class} sex {sex}: no outfit");
            }
        }
    }
}

#[test]
fn ported_bodies_read_through_the_chain() {
    let Some(data) = data() else {
        eprintln!("skipped: no WoW/Data/patch-W.MPQ");
        return;
    };
    let chain = Chain::open(&data).expect("open chain");
    for model in [
        "Character\\BloodElf\\Male\\BloodElfMale.m2",
        "Character\\BloodElf\\Female\\BloodElfFemale.m2",
        "Character\\Draenei\\Male\\DraeneiMale.m2",
        "Character\\Draenei\\Female\\DraeneiFemale.m2",
    ] {
        let bytes = chain.read(model).unwrap_or_else(|e| panic!("{model}: {e}"));
        let m2 = benilla_m2::parse_m2(&mut Cursor::new(bytes.as_slice())).unwrap_or_else(|e| panic!("{model}: {e}"));
        let model_ref = m2.model();
        assert!(model_ref.playable_animation_lookup.len() > 100, "{model}: playable lookup");
        assert!(model_ref.owns_animation(0) && model_ref.owns_animation(4), "{model}: Stand and Run");
        let anims = benilla_formats::parse_m2_animations(&bytes);
        let run = anims.iter().find(|a| a.anim_id == 4).expect("Run");
        assert!(run.duration > 0.1 && run.bones.iter().any(|b| b.rotation.len() > 2), "{model}: Run is keyed");
    }
}
#[test]
fn ported_skin_layers_are_at_the_1_12_sizes_the_compositor_paints() {
    let Some(data) = data() else {
        eprintln!("skipped: no WoW/Data/patch-W.MPQ");
        return;
    };
    let chain = Chain::open(&data).expect("open chain");
    for (path, size) in [
        ("Character\\BloodElf\\Male\\BloodElfMaleSkin00_00.blp", (256, 256)),
        ("Character\\BloodElf\\Female\\BloodElfFemaleFaceLower00_00.blp", (128, 64)),
        ("Character\\Draenei\\Male\\DraeneiMaleFaceUpper00_00.blp", (128, 32)),
        ("Character\\Draenei\\Female\\DraeneiFemaleNakedTorsoSkin00_00.blp", (128, 64)),
    ] {
        let d = benilla_blp::decode(&chain.read(path).unwrap_or_else(|e| panic!("{path}: {e}"))).expect(path);
        assert_eq!((d.width, d.height), size, "{path}");
    }
}