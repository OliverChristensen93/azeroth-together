//! The first frames on every connection, before any game message: the client says which game it
//! plays, and the host answers whether it plays the same one. Survivors and Nightfall share the
//! port and the invite code, so a friend who opens the wrong game is told so, not left hanging on
//! a lobby that never comes.

use serde::{Deserialize, Serialize};

/// Bumped whenever the greeting or the verdict changes shape.
pub(crate) const HANDSHAKE: u32 = 1;

/// The client's first frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Greeting {
    pub handshake: u32,
    /// The client's game, a [`crate::GameTag`].
    pub game: String,
}

/// The host's answer; anything but [`Verdict::Welcome`] is followed by the host closing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Verdict {
    Welcome,
    /// The host plays `host`.
    OtherGame { host: String },
    /// The client's handshake is another version.
    OtherHandshake,
}

/// The host's verdict on a greeting, for a host playing `game`.
pub(crate) fn judge(greeting: &Greeting, game: &str) -> Verdict {
    if greeting.handshake != HANDSHAKE {
        Verdict::OtherHandshake
    } else if greeting.game != game {
        Verdict::OtherGame {
            host: greeting_name(game),
        }
    } else {
        Verdict::Welcome
    }
}

/// What the joining player reads when the host refuses them, for a client playing `ours`.
pub(crate) fn refusal(verdict: &Verdict, ours: &str) -> String {
    match verdict {
        Verdict::Welcome => String::new(),
        Verdict::OtherGame { host } => format!(
            "That host is playing {host}, not {}.",
            greeting_name(ours)
        ),
        Verdict::OtherHandshake => "That host runs another version of the game.".into(),
    }
}

/// A game tag as players read it: "nightfall" is "Nightfall".
fn greeting_name(tag: &str) -> String {
    let mut chars = tag.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn greeting(game: &str) -> Greeting {
        Greeting {
            handshake: HANDSHAKE,
            game: game.into(),
        }
    }

    #[test]
    fn the_same_game_is_welcome() {
        assert_eq!(judge(&greeting("nightfall"), "nightfall"), Verdict::Welcome);
    }

    #[test]
    fn another_game_is_refused_with_the_hosts_name() {
        let v = judge(&greeting("nightfall"), "survivors");
        assert_eq!(
            v,
            Verdict::OtherGame {
                host: "Survivors".into()
            }
        );
        assert_eq!(
            refusal(&v, "nightfall"),
            "That host is playing Survivors, not Nightfall."
        );
    }

    #[test]
    fn another_handshake_is_refused() {
        let old = Greeting {
            handshake: HANDSHAKE + 1,
            game: "survivors".into(),
        };
        let v = judge(&old, "survivors");
        assert_eq!(v, Verdict::OtherHandshake);
        assert!(refusal(&v, "survivors").contains("another version"));
    }
}
