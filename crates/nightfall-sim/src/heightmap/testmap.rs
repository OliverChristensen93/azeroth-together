//! Small hand-made height maps for tests: flat ground and a house, in yards from the origin. No
//! WoW data is needed to test sight and paths on them (spec §12).

use super::{Grid, HeightMapBuilder, CELL};
use crate::V2;

/// A square grid `size` yards across, its corner at the origin.
pub(crate) fn grid(size: f32) -> Grid {
    let n = (size / CELL) as u32;
    Grid {
        origin: V2::ZERO,
        cell: CELL,
        nx: n,
        ny: n,
    }
}

/// Flat ground at height 0, `size` yards across.
pub(crate) fn flat(size: f32) -> HeightMapBuilder {
    HeightMapBuilder::flat(grid(size), 0.0)
}

/// A house over `[min, max]` on ground at height 0: walls half a yard thick and 3 yards tall, a
/// door 2.5 yards wide in the south wall (the one at `min.x`) centred at `door_y` when given, and
/// a roof slab on top when `roof`.
pub(crate) fn house(b: &mut HeightMapBuilder, min: V2, max: V2, door_y: Option<f32>, roof: bool) {
    const THICK: f32 = 0.5;
    const TALL: f32 = 3.0;
    // North, east and west walls.
    b.add_box(V2::new(max.x - THICK, min.y), max, 0.0, TALL);
    b.add_box(min, V2::new(max.x, min.y + THICK), 0.0, TALL);
    b.add_box(V2::new(min.x, max.y - THICK), max, 0.0, TALL);
    // The south wall, around its door.
    match door_y {
        None => b.add_box(min, V2::new(min.x + THICK, max.y), 0.0, TALL),
        Some(d) => {
            b.add_box(min, V2::new(min.x + THICK, d - 1.25), 0.0, TALL);
            b.add_box(
                V2::new(min.x, d + 1.25),
                V2::new(min.x + THICK, max.y),
                0.0,
                TALL,
            );
        }
    }
    if roof {
        b.add_box(min, max, TALL, TALL + 0.3);
    }
}
