//! Co-op for Nightfall. Task 9 of the foundation plan fills this in; until then a run is solo.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub(super) struct Net {
    pub role: Role,
    pub status: String,
}

#[derive(Default)]
pub(super) enum Role {
    #[default]
    Solo,
}

impl Net {
    pub fn is_solo(&self) -> bool {
        matches!(self.role, Role::Solo)
    }

    pub fn host(&mut self) {
        self.status = "Hosting arrives with the session code.".into();
    }

    pub fn join(&mut self, _addr: &str) {
        self.status = "Joining arrives with the session code.".into();
    }

    pub fn leave(&mut self, why: impl Into<String>) {
        self.role = Role::Solo;
        self.status = why.into();
    }

    /// The invite code, once ready.
    pub fn invite_code(&self) -> Option<String> {
        None
    }

    /// The lines the menu shows under the buttons: who is in the lobby and what is happening.
    pub fn lobby_text(&self) -> String {
        self.status.clone()
    }
}
