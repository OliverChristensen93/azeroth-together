//! Where a point of a placed model lands in the world, in WoW's frame (`x` north, `y` west, `z`
//! up). The rotation is the map's MDDF/MODF Euler triple in degrees, composed as the engine
//! composes it (the `in_wow` half of `benilla_assets::coords::placement_rotation`, wowdev.wiki's
//! `createPlacementMatrix`): `Rx(90°)·Ry(ry−180°)·Rz(−rx)·Rx(rz−90°)`, applied right to left.
//! Written out here because the engine's version is in Bevy's types, which the bake does not use.

/// The world point of model-local point `local` for a model at `position`, turned by
/// `rotation_deg` and scaled by `scale`.
pub fn placement_point(
    position: [f32; 3],
    rotation_deg: [f32; 3],
    scale: f32,
    local: [f32; 3],
) -> [f32; 3] {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (rx, ry, rz) = (
        rotation_deg[0].to_radians(),
        rotation_deg[1].to_radians(),
        rotation_deg[2].to_radians(),
    );
    let rot_x = |p: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
    };
    let rot_y = |p: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
    };
    let rot_z = |p: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
    };
    let p = [local[0] * scale, local[1] * scale, local[2] * scale];
    let p = rot_x(p, rz - FRAC_PI_2);
    let p = rot_z(p, -rx);
    let p = rot_y(p, ry - PI);
    let p = rot_x(p, FRAC_PI_2);
    [position[0] + p[0], position[1] + p[1], position[2] + p[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    #[test]
    fn a_heading_of_180_leaves_the_model_as_it_is() {
        let p = placement_point([10.0, 20.0, 30.0], [0.0, 180.0, 0.0], 1.0, [1.0, 2.0, 3.0]);
        assert!(close(p, [11.0, 22.0, 33.0]), "{p:?}");
    }

    #[test]
    fn a_heading_turns_about_up_and_keeps_up_up() {
        let turned = placement_point([0.0; 3], [0.0, 270.0, 0.0], 1.0, [1.0, 0.0, 0.0]);
        assert!(close(turned, [0.0, 1.0, 0.0]), "{turned:?}");
        let up = placement_point([0.0; 3], [0.0, 270.0, 0.0], 1.0, [0.0, 0.0, 1.0]);
        assert!(close(up, [0.0, 0.0, 1.0]), "{up:?}");
    }

    #[test]
    fn scale_grows_the_model_about_its_origin() {
        let p = placement_point([5.0, 0.0, 0.0], [0.0, 180.0, 0.0], 2.0, [1.0, 0.0, 0.0]);
        assert!(close(p, [7.0, 0.0, 0.0]), "{p:?}");
    }
}
