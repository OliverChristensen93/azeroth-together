//! A game's wind for the weather (fork carry, in its own file): 1.12's rain and mist lean only
//! with the player's own motion ([`super::precip`]'s `WeatherWind`), so a wind is fed in as
//! the player drifting against it. Absent or zero, the weather is the reference's.

use bevy::prelude::*;

/// The wind blowing through the weather, Bevy space, yards per second (planar).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherGust {
    pub wind: Vec3,
    /// The air's travel so far: the player seen from the air has moved by minus this.
    drift: Vec3,
}

impl WeatherGust {
    /// Advance the air by `dt` and return the offset to take from the player's position.
    pub(crate) fn advance(&mut self, dt: f32) -> Vec3 {
        self.drift += self.wind.with_y(0.0) * dt;
        self.drift
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_air_drifts_with_the_wind() {
        let mut g = WeatherGust { wind: Vec3::new(4.0, 9.0, 0.0), ..Default::default() };
        g.advance(0.5);
        assert_eq!(g.advance(0.5), Vec3::new(4.0, 0.0, 0.0));
    }
}
