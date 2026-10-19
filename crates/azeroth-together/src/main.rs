//! The `azeroth-together` launcher: the build stamp, handed to the co-op survivors mode.

use std::path::{Path, PathBuf};

use benilla_app::BuildId;

fn main() -> benilla_app::AppExit {
    let project_dir = env!("BENILLA_PROJECT_DIR");
    worktree_data(Path::new(project_dir));
    benilla_app::run_together(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir,
        ..Default::default()
    })
}

/// A dev build looks for the game data in the folder it was built from (`WoW/Data`). A linked git
/// worktree has none, as the data is gitignored and lives in the primary checkout: there, with no
/// `$WOW_DATA` set, it borrows the primary checkout's, so a branch built in its own worktree runs
/// like the main one instead of drawing only the sky.
fn worktree_data(project: &Path) {
    if std::env::var_os("WOW_DATA").is_some() || project.join("WoW/Data").is_dir() {
        return;
    }
    if let Some(data) = primary_checkout(project)
        .map(|p| p.join("WoW/Data"))
        .filter(|d| d.is_dir())
    {
        // Before any other thread exists.
        std::env::set_var("WOW_DATA", &data);
    }
}

/// The primary checkout of the worktree at `project`: its `.git` is a file pointing at
/// `<primary>/.git/worktrees/<name>`.
fn primary_checkout(project: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(project.join(".git")).ok()?;
    let gitdir = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
    // …/.git/worktrees/<name> → …/.git → the primary checkout.
    gitdir.ancestors().nth(3).map(Path::to_path_buf)
}
