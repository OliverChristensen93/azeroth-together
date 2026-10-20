//! Read and flash access to the resolved [`WowLighting`] for a game's own atmosphere layer (god
//! rays, lightning), which lives outside this crate. A fork carry in its own file: the fields stay
//! `pub(crate)`, these are the only doors.

use bevy::prelude::*;

use super::WowLighting;

impl WowLighting {
    /// The visible sun, camera to sun in Bevy space; it rises and sets, unlike [`Self::sun_dir`].
    pub fn celestial_dir(&self) -> Vec3 {
        self.celestial_dir
    }

    /// The sun's lens-flare day envelope: 1 by day, 0 at night.
    pub fn sun_day(&self) -> f32 {
        self.sun_flare_dn
    }

    /// The white moon, camera to moon in Bevy space.
    pub fn moon_dir(&self) -> Vec3 {
        self.moon_dir_white
    }

    /// The moon's lens-flare night envelope: 1 after midnight, 0 by day.
    pub fn moon_night(&self) -> f32 {
        self.moon_flare_dn
    }

    /// The horizon stop of the sky dome, sRGB 0..1.
    pub fn horizon_color(&self) -> [f32; 3] {
        self.sky[4]
    }

    /// A lightning flash: every light, the sky and both fogs lerped toward `tint` by `k` (0..1).
    /// Call between `LightingResolveSet` and `LightingConsumeSet`, once per frame.
    pub fn flash(&mut self, k: f32, tint: [f32; 3]) {
        if k <= 0.0 {
            return;
        }
        let k = k.min(1.0);
        let mix = |c: &mut [f32; 3]| {
            for i in 0..3 {
                c[i] += (tint[i] - c[i]) * k;
            }
        };
        mix(&mut self.ambient);
        mix(&mut self.diffuse);
        mix(&mut self.fog_color);
        mix(&mut self.wmo_fog_color);
        for stop in &mut self.sky {
            mix(stop);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_flash_reaches_the_tint_and_none_leaves_it() {
        let mut l = WowLighting {
            ambient: [0.1, 0.1, 0.1],
            sky: [[0.2; 3]; 5],
            ..default()
        };
        let before = l;
        l.flash(0.0, [1.0; 3]);
        assert!(l == before);
        l.flash(1.0, [0.9, 0.95, 1.0]);
        assert_eq!(l.ambient, [0.9, 0.95, 1.0]);
        assert_eq!(l.horizon_color(), [0.9, 0.95, 1.0]);
    }
}
