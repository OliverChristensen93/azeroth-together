//! The senses (spec §5): how visible and how loud each hero is, what each enemy makes of them,
//! and what it does about it. The host's alone: a client's mirror gets each hero's visibility and
//! noise in the snapshot, and its own player's marks beside it.

mod noise;
mod visibility;

pub use noise::Loudness;
pub(crate) use noise::Noise;
pub use visibility::{Gait, Stance};
