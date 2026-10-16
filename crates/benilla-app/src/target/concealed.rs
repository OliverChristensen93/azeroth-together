//! A unit the player is not to know of though it stands in view, as a game mode's own rules decide
//! (Nightfall hides the enemies its hero does not see). TAB ([`super::scan`]) and the mouseover
//! pick ([`super::hover`]) skip it, and no plate or name floats over it; its body is still drawn.

use bevy::prelude::*;

use crate::net::ObjectStore;

/// Marks a unit kept out of targeting and plates.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct Concealed;

/// Whether a unit has no plate and no name: concealed, or `UNIT_FIELD_BYTES_1` byte 3 `& 3`
/// (ghost or creep), as `0x607101` and `0x60f62e` test it.
pub(crate) fn plate_hidden(store: Option<&ObjectStore>, concealed: bool) -> bool {
    concealed || store.is_some_and(|s| s.0.unit_is_ghost_visual() || s.0.unit_is_stealthed())
}

#[cfg(test)]
mod tests {
    use benilla_protocol::field::FIELD_UNIT_BYTES_1;
    use benilla_protocol::messages::ObjectFields;

    use super::*;

    fn store(bytes_1: u32) -> ObjectStore {
        ObjectStore(ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, bytes_1)]))
    }

    #[test]
    fn concealed_stealthed_and_ghostly_units_have_no_plate() {
        assert!(!plate_hidden(Some(&store(0)), false));
        assert!(plate_hidden(Some(&store(0)), true));
        assert!(plate_hidden(Some(&store(0x0100_0000)), false));
        assert!(plate_hidden(Some(&store(0x0200_0000)), false));
        // Untrackable hides the minimap dot, not the plate.
        assert!(!plate_hidden(Some(&store(0x0400_0000)), false));
        assert!(plate_hidden(None, true));
        assert!(!plate_hidden(None, false));
    }
}
