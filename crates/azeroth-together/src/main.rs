//! The `azeroth-together` launcher: the build stamp, handed to the co-op survivors mode.

use std::path::Path;

use benilla_app::BuildId;

fn main() -> benilla_app::AppExit {
    benilla_app::run_together(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        describe: env!("BENILLA_GIT_DESCRIBE"),
        sha: env!("BENILLA_GIT_SHA"),
        short: env!("BENILLA_GIT_SHORT"),
        date: env!("BENILLA_GIT_DATE"),
        profile: env!("BENILLA_PROFILE"),
        project_dir: project_dir(),
        ..Default::default()
    })
}

/// The stamped project folder, unless it has no `WoW\Data`: checkouts that share one `target`
/// (a worktree building into the main checkout's) share one cached build-script result, so the
/// stamp can name another checkout. Then the checkout this binary sits in,
/// `<root>\target\<profile>\azeroth-together.exe`. A player build's empty stamp stays empty.
fn project_dir() -> &'static str {
    let stamped = env!("BENILLA_PROJECT_DIR");
    if stamped.is_empty() || Path::new(stamped).join("WoW/Data").is_dir() {
        return stamped;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(Path::to_path_buf))
        .filter(|root| root.join("WoW/Data").is_dir())
        .map_or(stamped, |root| Box::leak(root.display().to_string().into_boxed_str()))
}