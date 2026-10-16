//! The content Nightfall ships (`content/nightfall/`) loads and holds what the game expects.

use nightfall_sim::{Content, ContentSources, V2};

pub fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/nightfall/classes.ron"),
        map: include_str!("../../../content/nightfall/map.ron"),
        rules: include_str!("../../../content/nightfall/rules.ron"),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_shipped_content_loads_with_rogue_and_mage_in_duskwood() {
    let c = shipped();
    assert!(c.class("rogue").is_some());
    assert!(c.class("mage").is_some());
    assert_eq!(c.map.key, "duskwood");
    assert_eq!(c.map.map, 0);
    for e in &c.map.entries {
        assert!(c.map.bounds.contains(V2::new(e[0], e[1])));
    }
}
