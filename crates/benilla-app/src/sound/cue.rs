//! A game's own sounds: a kit by SoundEntries id or name, or a file, from a place in the world or
//! everywhere at once. The Lua `PlaySound` seam ([`super::ui`]) is 2D only, and no server sends a
//! game that is its own server (Nightfall) the sounds of its world: a monster's tread or a
//! flock taking off must come from where it happens. A file plays 2D whatever `at` says, as the
//! client's `PlaySoundFile` does; a kit carries the distances its SoundEntries row gives.

use bevy::prelude::*;

use benilla_assets::WorldAssets;
use benilla_world::schedule::WorldStage;

use super::kit::{self, KitRef, SoundCategory, SoundKits};
use super::{AudioListener, SoundConfig, SoundOutput};

/// What a [`SoundCue`] plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Cue {
    Kit(u32),
    Name(String),
    File(String),
}

/// Play `sound`: from `at` (Bevy space) when given, else 2D. On the SFX slider.
#[derive(Message, Clone, Debug)]
pub(crate) struct SoundCue {
    pub(crate) sound: Cue,
    pub(crate) at: Option<Vec3>,
}

/// Every cue of the frame into the kit player; drained even with no catalog, so none waits.
fn play_cues(
    mut cues: MessageReader<SoundCue>,
    kits: Option<ResMut<SoundKits>>,
    assets: Option<Res<WorldAssets>>,
    mut out: NonSendMut<SoundOutput>,
    config: Res<SoundConfig>,
    listener: Res<AudioListener>,
) {
    let (Some(mut kits), Some(assets)) = (kits, assets) else {
        cues.clear();
        return;
    };
    for cue in cues.read() {
        let played = match &cue.sound {
            Cue::File(path) => kit::play_file(
                &mut kits,
                &assets,
                &mut out,
                &config,
                path,
                SoundCategory::Sfx,
            ),
            Cue::Kit(id) => kit::play_kit(
                &mut kits,
                &assets,
                &mut out,
                &config,
                listener.pos,
                KitRef::Id(*id),
                cue.at,
                SoundCategory::Sfx,
            )
            .map(|_| ()),
            Cue::Name(name) => kit::play_kit(
                &mut kits,
                &assets,
                &mut out,
                &config,
                listener.pos,
                KitRef::Name(name),
                cue.at,
                SoundCategory::Sfx,
            )
            .map(|_| ()),
        };
        if let Err(e) = played {
            debug!("sound(cue): {cue:?} — {e:#}");
        }
    }
}

/// Registration hook for [`super::SoundPlugin`].
pub(super) fn plugin(app: &mut App) {
    app.add_message::<SoundCue>()
        .add_systems(Update, play_cues.in_set(WorldStage::Present));
}
