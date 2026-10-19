//! One stash per home: `stash.ron.lock` beside the stash holds the process that is using it (its
//! id and when the lock was taken, two numbers on two lines). The first game takes it; a second
//! on the same home finds a live holder and gets a stash it may read and not write, so no raids:
//! each would settle the other's raid under way as missing, and the last writer would win.
//!
//! The lock is taken by creating it new, so two starts together cannot both get it. A holder that
//! is gone (a crash left the file) is found out by its id: no such process, or one that began
//! after the lock was taken (the id was handed out again). That check is Windows' own
//! `OpenProcess`/`GetProcessTimes` and, elsewhere, `kill(pid, 0)`, which cannot tell a reused id
//! (there a stale lock whose id was reused holds until that process ends). Dropping the lock
//! removes the file, best effort: a game killed outright leaves a stale lock, which the next
//! start takes over. The web build has no second process, and no lock.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A lock file that does not read as two numbers, younger than this, is a lock being written by
/// another start; older, it is rubbish left behind.
const BEING_WRITTEN: Duration = Duration::from_secs(5);

/// What a lock file says about who may take it.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// No lock: take it.
    Free,
    /// A lock nobody holds (its process is gone, or it is ours from before): take it over.
    Stale,
    /// A live process is using the stash (id 0: a lock being written that cannot be read yet).
    Held { pid: u32 },
}

/// The decision on the lock file's text (`None`: no file), `me` being this process, the file's
/// `age`, and `alive(pid, taken)` whether the process `pid` is running and is the one that took
/// the lock at unix second `taken`.
pub(super) fn judge(
    text: Option<&str>,
    me: u32,
    age: Duration,
    alive: impl Fn(u32, u64) -> bool,
) -> Verdict {
    let Some(text) = text else {
        return Verdict::Free;
    };
    let mut lines = text.lines();
    let pid = lines.next().and_then(|l| l.trim().parse::<u32>().ok());
    let taken = lines.next().and_then(|l| l.trim().parse::<u64>().ok());
    match (pid, taken) {
        (Some(pid), Some(taken)) if pid != me && alive(pid, taken) => Verdict::Held { pid },
        (Some(_), Some(_)) => Verdict::Stale,
        _ if age < BEING_WRITTEN => Verdict::Held { pid: 0 },
        _ => Verdict::Stale,
    }
}

/// The stash is ours until this is dropped.
#[derive(Debug)]
pub(super) struct StashLock {
    path: PathBuf,
}

impl Drop for StashLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Why the lock was not taken.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refused {
    /// A live game is using the stash.
    Held { pid: u32 },
    /// The lock could not be made (a read-only folder): the stash is used without one, as it was
    /// before there were locks, rather than shut out for a file the player cannot fix.
    Unavailable(String),
}

/// Takes the lock at `lock`, the folder made if need be.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn acquire(lock: &Path) -> Result<StashLock, Refused> {
    use std::io::Write;
    let me = std::process::id();
    let now = unix_secs();
    if let Some(dir) = lock.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    for _ in 0..3 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock)
        {
            Ok(mut f) => {
                // One write of the whole text; a reader that sees it half-made waits it out
                // (see `BEING_WRITTEN`).
                return match f.write_all(format!("{me}\n{now}\n").as_bytes()) {
                    Ok(()) => Ok(StashLock {
                        path: lock.to_path_buf(),
                    }),
                    Err(e) => {
                        drop(f);
                        let _ = std::fs::remove_file(lock);
                        Err(Refused::Unavailable(e.to_string()))
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(Refused::Unavailable(e.to_string())),
        }
        let text = std::fs::read_to_string(lock).ok();
        let age = std::fs::metadata(lock)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .unwrap_or_default();
        match judge(text.as_deref(), me, age, process_alive) {
            Verdict::Held { pid } => return Err(Refused::Held { pid }),
            // Someone took it between our attempt and the read: try again.
            Verdict::Free => {}
            Verdict::Stale => {
                // Remove it only if it is still the stale lock we judged, not a new one a
                // faster start has made since.
                if std::fs::read_to_string(lock).ok() == text {
                    let _ = std::fs::remove_file(lock);
                }
            }
        }
    }
    Err(Refused::Unavailable(
        "the lock kept changing under us".into(),
    ))
}

/// Seconds since the unix epoch.
fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Whether process `pid` is running and began no later than the lock was taken at `taken` (a
/// process that began after it is not the one that took it, the id having been handed out again).
#[cfg(all(windows, not(target_arch = "wasm32")))]
fn process_alive(pid: u32, taken: u64) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    /// What `GetExitCodeProcess` says of a process still running.
    const STILL_ACTIVE: u32 = 259;
    /// Seconds between 1601 (a `FILETIME`'s epoch) and 1970.
    const EPOCH_GAP: u64 = 11_644_473_600;
    // SAFETY: the handle is closed before returning; `GetExitCodeProcess` and `GetProcessTimes`
    // only write the plain integers passed to them.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // A process we may not open is still a process.
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let mut running = GetExitCodeProcess(handle, &mut code) != 0 && code == STILL_ACTIVE;
        if running {
            let mut creation = std::mem::zeroed();
            let mut exit = std::mem::zeroed();
            let mut kernel = std::mem::zeroed();
            let mut user = std::mem::zeroed();
            if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
                let ticks =
                    (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
                let began = (ticks / 10_000_000).saturating_sub(EPOCH_GAP);
                // A little slack: the lock is stamped after the process began, never before.
                running = began <= taken.saturating_add(2);
            }
        }
        CloseHandle(handle);
        running
    }
}

/// Whether process `pid` is running. A reused id cannot be told from the holder here.
#[cfg(all(unix, not(target_arch = "wasm32")))]
fn process_alive(pid: u32, _taken: u64) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the process exists.
    let r = unsafe { libc::kill(pid, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// No way to tell on another platform: the holder is taken to be gone, so the lock never shuts a
/// player out of their stash for good.
#[cfg(not(any(windows, unix, target_arch = "wasm32")))]
fn process_alive(_pid: u32, _taken: u64) -> bool {
    false
}

/// The web build has one page and no lock.
#[cfg(target_arch = "wasm32")]
pub(super) fn acquire(_lock: &Path) -> Result<StashLock, Refused> {
    Err(Refused::Unavailable("no lock on the web".into()))
}

/// The lock file beside the stash at `stash`.
pub(super) fn lock_path(stash: &Path) -> PathBuf {
    let mut name = stash.file_name().unwrap_or_default().to_os_string();
    name.push(".lock");
    stash.with_file_name(name)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    const OLD: Duration = Duration::from_secs(600);

    fn alive_only(live: u32) -> impl Fn(u32, u64) -> bool {
        move |pid, _| pid == live
    }

    #[test]
    fn no_lock_is_free_and_a_live_holder_holds_it() {
        assert_eq!(judge(None, 7, OLD, alive_only(0)), Verdict::Free);
        assert_eq!(
            judge(Some("1234\n1000\n"), 7, OLD, alive_only(1234)),
            Verdict::Held { pid: 1234 }
        );
    }

    #[test]
    fn a_holder_that_is_gone_leaves_a_stale_lock() {
        assert_eq!(
            judge(Some("1234\n1000\n"), 7, OLD, alive_only(99)),
            Verdict::Stale
        );
        // The id was handed out again: the check is told when the lock was taken.
        assert_eq!(
            judge(Some("1234\n1000\n"), 7, OLD, |_, taken| taken > 5000),
            Verdict::Stale
        );
    }

    #[test]
    fn our_own_id_is_never_a_holder() {
        // A lock from an earlier run of ours that had the same id.
        assert_eq!(
            judge(Some("7\n1000\n"), 7, OLD, |_, _| true),
            Verdict::Stale
        );
    }

    #[test]
    fn a_lock_that_does_not_read_is_being_written_or_rubbish() {
        for text in ["", "7", "x\ny\n", "7\nsoon\n"] {
            assert_eq!(
                judge(Some(text), 1, Duration::from_secs(1), |_, _| false),
                Verdict::Held { pid: 0 },
                "{text:?} just made"
            );
            assert_eq!(
                judge(Some(text), 1, OLD, |_, _| false),
                Verdict::Stale,
                "{text:?} long ago"
            );
        }
    }

    fn folder(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-lock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_lock_is_taken_with_our_id_and_released_on_drop() {
        let lock = folder("take").join("nightfall").join("stash.ron.lock");
        let ours = acquire(&lock).unwrap();
        let text = std::fs::read_to_string(&lock).unwrap();
        assert_eq!(
            text.lines().next(),
            Some(std::process::id().to_string().as_str())
        );
        drop(ours);
        assert!(!lock.exists());
    }

    #[test]
    fn a_stale_lock_is_taken_over_and_dropping_removes_it() {
        let lock = folder("stale").join("stash.ron.lock");
        // A process that has certainly ended: one we ran and waited for.
        #[cfg(windows)]
        let mut cmd = std::process::Command::new("cmd");
        #[cfg(windows)]
        cmd.args(["/C", "exit", "0"]);
        #[cfg(not(windows))]
        let mut cmd = std::process::Command::new("true");
        let mut gone = cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = gone.id();
        gone.wait().unwrap();
        std::fs::write(&lock, format!("{pid}\n{}\n", unix_secs())).unwrap();
        let taken = acquire(&lock).expect("a lock whose holder ended is taken over");
        assert_eq!(
            std::fs::read_to_string(&lock).unwrap().lines().next(),
            Some(std::process::id().to_string().as_str())
        );
        drop(taken);
        assert!(!lock.exists(), "released");
    }

    #[test]
    fn a_live_holder_keeps_the_lock() {
        let lock = folder("live").join("stash.ron.lock");
        // A process that is running for the length of the test: a child that waits.
        let mut child = long_lived_child();
        std::fs::write(&lock, format!("{}\n{}\n", child.id(), unix_secs())).unwrap();
        let refused = acquire(&lock).unwrap_err();
        assert_eq!(refused, Refused::Held { pid: child.id() });
        assert!(lock.exists(), "a held lock is left alone");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn the_lock_sits_beside_the_stash() {
        assert_eq!(
            lock_path(Path::new("a/b/stash.ron")),
            PathBuf::from("a/b/stash.ron.lock")
        );
    }

    /// A child process that stays up until killed.
    fn long_lived_child() -> std::process::Child {
        #[cfg(windows)]
        let mut c = std::process::Command::new("ping");
        #[cfg(windows)]
        c.args(["-n", "60", "127.0.0.1"]);
        #[cfg(not(windows))]
        let mut c = std::process::Command::new("sleep");
        #[cfg(not(windows))]
        c.arg("60");
        c.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }
}
