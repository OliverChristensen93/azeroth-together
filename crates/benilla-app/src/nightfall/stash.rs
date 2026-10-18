//! The player's things between raids, on disk: `benilla-config/nightfall/stash.ron`, a
//! [`Profile`] in RON (spec §8.3: each player's stash on their own machine). Never lost silently:
//! every save keeps the last good file as `stash.ron.bak`; a file that does not read is kept as
//! `stash.ron.broken` (numbered when there is one already) and the backup tried; a damaged backup
//! is kept as `stash.ron.bak-broken` before a save can replace it; a file or a backup from a newer
//! Nightfall is left untouched, and this session does not save. Whatever happened is said on the
//! menu ([`Stash::note`]).
//!
//! A raid is written to disk before it starts ([`Stash::begin_raid`]): the loadout leaves the
//! profile for `in_raid`, and a raid whose begin cannot be saved does not start, or it would be a
//! copy of the loadout the next crash leaves on disk. A raid that never ends (a crash, a lost
//! connection, the host going) is found there at the next start and settled as missing, once
//! ([`Stash::open`], [`after_raid`]); a raid that ends is settled by its outcome ([`settle_run`])
//! and saved at once.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use nightfall_sim::{Content, Loadout, Profile, ProfileError, SavedStack};

use super::content;
use super::run::Sim;

/// The stash as read, and what to say about it.
pub(super) struct Loaded {
    pub profile: Profile,
    pub note: Option<String>,
    /// Whether this session may save over the file.
    pub writable: bool,
    /// The profile was read from a file the player has (not a fresh default): a session that may
    /// not save it must not play raids with it, or the haul would be lost and the loadout copied.
    pub real: bool,
    /// The stash file was damaged and a copy is kept aside: once the session is known to save, it
    /// is replaced or removed ([`load`]), so no copy is made at every launch.
    pub kept_main: bool,
}

fn backup_of(path: &Path) -> PathBuf {
    path.with_extension("ron.bak")
}

/// How many files of one kind are set aside before there is no more room for another.
const MAX_ASIDE: u32 = 999;

/// Where a damaged file is set aside: `stash.ron.<tag>`, or the first `stash.ron.<tag>-N` free.
/// None when `MAX_ASIDE` are kept already: a copy is never put over an earlier one, so the file
/// is not touched and the session does not save.
fn aside_of(path: &Path, tag: &str) -> Option<PathBuf> {
    let free = |p: &Path| matches!(crate::local_state::read_to_string(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
    let first = path.with_extension(format!("ron.{tag}"));
    if free(&first) {
        return Some(first);
    }
    (2..=MAX_ASIDE)
        .map(|n| path.with_extension(format!("ron.{tag}-{n}")))
        .find(|p| free(p))
}

/// What the backup file is.
enum Backup {
    Absent,
    Good(Profile),
    /// From a newer Nightfall: a save would replace it with an older shape.
    Newer,
    Damaged {
        text: String,
        why: String,
    },
    Unreadable(String),
}

fn read_backup(path: &Path) -> Backup {
    match crate::local_state::read_to_string(&backup_of(path)) {
        Ok(text) => match Profile::from_ron(&text) {
            Ok(p) => Backup::Good(p),
            Err(ProfileError::Newer { .. }) => Backup::Newer,
            Err(ProfileError::Parse(why)) => Backup::Damaged { text, why },
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Backup::Absent,
        Err(e) => Backup::Unreadable(e.to_string()),
    }
}

/// `text` kept beside the stash as `stash.ron.<tag>[-N]`: its file name, or None when it could not
/// be (no room, or the write failed).
fn keep_aside(path: &Path, tag: &str, text: &str) -> Option<String> {
    let aside = aside_of(path, tag)?;
    crate::local_state::write_atomic(&aside, text).ok()?;
    Some(
        aside
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

/// A damaged backup, already kept aside, put out of the way: replaced by the stash file's text
/// when that is a good stash (atomically), else removed. Whether that was done; if not the
/// backup stays and is kept aside again next time.
fn replace_backup(path: &Path) -> bool {
    let bak = backup_of(path);
    match crate::local_state::read_to_string(path) {
        Ok(text) if Profile::from_ron(&text).is_ok() => {
            crate::local_state::write_atomic(&bak, &text).is_ok()
        }
        _ => crate::local_state::remove_file(&bak).is_ok(),
    }
}

/// The stash at `path`, and its backup, which a save would replace. A backup from a newer
/// Nightfall, or one that cannot be read, or one damaged and not to be kept aside, makes the
/// session read-only; a damaged one that was kept aside is just said. When the stash file itself
/// already makes the session read-only the backup is left alone, so no copy of it piles up at
/// every launch.
pub(super) fn load(path: &Path) -> Loaded {
    let backup = read_backup(path);
    let good = match &backup {
        Backup::Good(p) => Some(p.clone()),
        _ => None,
    };
    // A backup that will make the session read-only (newer, unreadable) leaves a damaged stash
    // file where it is: nothing is copied aside by a session that will not save.
    let saves = !matches!(backup, Backup::Newer | Backup::Unreadable(_));
    let mut loaded = load_main(path, good, saves);
    if !loaded.writable {
        return loaded;
    }
    // The stash file is missing and nothing came back from a backup: a new stash begins.
    let begins = !loaded.real && loaded.note.is_none();
    let mut notes: Vec<String> = Vec::new();
    match backup {
        Backup::Good(_) | Backup::Absent => {}
        Backup::Newer => {
            loaded.writable = false;
            notes.push("Your stash's backup is from a newer Nightfall: it is left untouched, and this session does not save.".into());
        }
        Backup::Unreadable(e) => {
            loaded.writable = false;
            notes.push(format!("Your stash's backup could not be read ({e}); it is left as it is, and this session does not save."));
        }
        Backup::Damaged { text, why } => match keep_aside(path, "bak-broken", &text) {
            Some(kept) => {
                // The copy is safe: the damaged backup is replaced now, by the good stash file,
                // or removed when there is none. Left, every launch that does not save would
                // copy it aside again, and the copies would pile up.
                let replaced = replace_backup(path);
                notes.push(format!(
                    "Your stash's backup was damaged ({why}): it was kept as {kept}{}.",
                    if replaced { ", and replaced" } else { "" }
                ));
            }
            None => {
                loaded.writable = false;
                notes.push(format!("Your stash's backup is damaged ({why}) and could not be set aside: it is left as it is, and this session does not save."));
            }
        },
    }
    if begins {
        if let Some(last) = notes.last_mut() {
            last.push_str(" With no stash file, a new, empty stash begins.");
        }
    }
    if let Some(n) = loaded.note.take() {
        notes.push(n);
    }
    // The damaged stash file is safe aside, and the session saves: the file is replaced by the
    // stash that came back from the backup, or removed when nothing did (a fresh stash then
    // saves normally). Left, every launch that does not save would copy it aside again.
    if loaded.writable && loaded.kept_main {
        let _ = if loaded.real {
            crate::local_state::write_atomic(path, &loaded.profile.to_ron())
        } else {
            crate::local_state::remove_file(path)
        };
    }
    loaded.note = (!notes.is_empty()).then(|| notes.join("\n"));
    loaded
}

/// The stash file at `path`, with `backup` (the backup's profile, when it reads) to fall back on.
/// A damaged file is kept aside only when `may_keep_aside` (the session will save).
fn load_main(path: &Path, backup: Option<Profile>, may_keep_aside: bool) -> Loaded {
    let fresh = |note: Option<String>, writable: bool| Loaded {
        profile: Profile::default(),
        note,
        real: false,
        kept_main: false,
        writable,
    };
    let text = match crate::local_state::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return match backup {
                Some(profile) => Loaded {
                    profile,
                    note: Some("Your stash file was missing; it came back from its backup.".into()),
                    writable: true,
                    real: true,
                    kept_main: false,
                },
                None => fresh(None, true),
            };
        }
        Err(e) => {
            return fresh(
                Some(format!(
                    "Your stash could not be read ({e}); it is left as it is, and this session does not save."
                )),
                false,
            );
        }
    };
    match Profile::from_ron(&text) {
        Ok(profile) => Loaded {
            profile,
            note: None,
            writable: true,
            real: true,
            kept_main: false,
        },
        Err(ProfileError::Newer { .. }) => fresh(
            Some(
                "Your stash is from a newer Nightfall: it is left untouched, and this session's raids are not saved."
                    .into(),
            ),
            false,
        ),
        Err(ProfileError::Parse(why)) => {
            let kept = may_keep_aside.then(|| keep_aside(path, "broken", &text)).flatten();
            let Some(kept) = kept else {
                return fresh(
                    Some(format!(
                        "Your stash file is damaged ({why}) and could not be set aside: it is left as it is, and this session does not save."
                    )),
                    false,
                );
            };
            match backup {
                Some(profile) => Loaded {
                    profile,
                    note: Some(format!(
                        "Your stash file was damaged ({why}): it was kept as {kept}, and your stash came back from its backup."
                    )),
                    writable: true,
                    real: true,
                    kept_main: true,
                },
                None => Loaded {
                    kept_main: true,
                    ..fresh(
                        Some(format!(
                            "Your stash file was damaged ({why}) and had no good backup: it was kept as {kept}, and a new stash begins."
                        )),
                        true,
                    )
                },
            }
        }
    }
}

/// `profile` written to `path`, the file it replaces kept as the backup when it was a good one
/// (a damaged file never overwrites a good backup).
pub(super) fn save(path: &Path, profile: &Profile) -> std::io::Result<()> {
    if let Ok(old) = crate::local_state::read_to_string(path) {
        if Profile::from_ron(&old).is_ok() {
            crate::local_state::write_atomic(&backup_of(path), &old)?;
        }
    }
    crate::local_state::write_atomic(path, &profile.to_ron())
}

/// `NIGHTFALL_GIVE`'s text (`item:count,item:count`, a test hook) as stacks, and a line of warning
/// for each entry that is not one: no colon, a count that is not a whole number above 0, or an
/// item the content does not have. The rest are still given.
fn parse_give(text: &str, content: &Content) -> (Vec<SavedStack>, Vec<String>) {
    let (mut stacks, mut warnings) = (Vec::new(), Vec::new());
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((item, count)) = part.split_once(':') else {
            warnings.push(format!("\"{part}\" is not item:count"));
            continue;
        };
        let (item, count) = (item.trim(), count.trim());
        if content.item(item).is_none() {
            warnings.push(format!("\"{part}\": no item \"{item}\" in the content"));
            continue;
        }
        match count.parse::<u16>() {
            Ok(count) if count > 0 => stacks.push(SavedStack {
                item: item.to_owned(),
                count,
            }),
            _ => warnings.push(format!(
                "\"{part}\": the count is not a whole number from 1 to {}",
                u16::MAX
            )),
        }
    }
    (stacks, warnings)
}

/// The player's stash, as this session has it.
#[derive(Resource)]
pub(super) struct Stash {
    pub profile: Profile,
    path: Option<PathBuf>,
    writable: bool,
    /// This session may not save, and the profile is the player's real stash (a newer, unreadable or
    /// unkeepable backup holds the save back): their raids are off, not played unsaved.
    keeps_real: bool,
    /// What the menu says about the stash: a file that did not read, a raid counted missing, a
    /// move refused.
    pub note: Option<String>,
}

impl Stash {
    /// The stash from `benilla-config/nightfall/stash.ron`; a raid that never ended settled as
    /// missing; and `NIGHTFALL_GIVE`'s things added (a test hook). With no state folder (a
    /// capture) there is nothing to read and nothing is saved. A unit test never reaches a real
    /// stash, whatever `BENILLA_HOME` says: it gets a fresh one in memory.
    pub fn open() -> Self {
        let content = content();
        if cfg!(test) {
            return Self::open_at(None, &content);
        }
        let path = crate::local_state::home().map(|h| h.join("nightfall").join("stash.ron"));
        let mut stash = Self::open_at(path, &content);
        if let Ok(give) = std::env::var("NIGHTFALL_GIVE") {
            let (stacks, warnings) = parse_give(&give, &content);
            for w in warnings {
                warn!("nightfall: NIGHTFALL_GIVE: {w}");
            }
            for s in stacks {
                stash.profile.stash_add(s, &content);
            }
            stash.save();
        }
        stash
    }

    /// The stash at `path` (none: nothing read, nothing saved), a raid it still holds as under way
    /// settled as missing and saved at once: a game that closed in a raid did not leave it, so the
    /// raid ended without its player.
    fn open_at(path: Option<PathBuf>, content: &Content) -> Self {
        let loaded = match &path {
            Some(p) => load(p),
            None => Loaded {
                profile: Profile::default(),
                note: None,
                writable: false,
                real: false,
                kept_main: false,
            },
        };
        let mut stash = Stash {
            profile: loaded.profile,
            path,
            writable: loaded.writable,
            keeps_real: loaded.real && !loaded.writable,
            note: loaded.note,
        };
        if stash.profile.settle_missing(content) {
            stash.say("Your last raid ended without you: you were counted missing.");
            stash.save();
        }
        stash
    }

    /// `line` on the menu, under what it already says (once: asking again adds nothing). Whether
    /// it was added.
    fn say(&mut self, line: &str) -> bool {
        match self.note.take() {
            Some(old) if old.lines().any(|l| l == line) => {
                self.note = Some(old);
                false
            }
            Some(old) => {
                self.note = Some(format!("{old}\n{line}"));
                true
            }
            None => {
                self.note = Some(line.to_owned());
                true
            }
        }
    }

    /// The profile written to disk: whether anything was, or why it could not be. A session that
    /// may not save (a newer or unreadable file), or has no folder, writes nothing and is not
    /// an error.
    fn write(&self) -> std::io::Result<bool> {
        match (&self.path, self.writable) {
            (Some(path), true) => save(path, &self.profile).map(|()| true),
            _ => Ok(false),
        }
    }

    /// Written to disk, unless this session may not (a newer or unreadable file). Whether it is
    /// written, or there was nothing to write to; a write that failed is said in the note.
    pub fn save(&mut self) -> bool {
        match self.write() {
            Ok(written) => written,
            Err(e) => {
                warn!("nightfall: stash not saved: {e}");
                self.note = Some(format!("Your stash could not be saved: {e}"));
                false
            }
        }
    }

    /// A raid begins for a hero of `class`: their loadout leaves for it, and the profile says so on
    /// disk at once, before the raid starts. A raid that never ends is then settled as missing at
    /// the next start, never kept twice. `told` is the loadout the raid is told they bring (the
    /// pick the host got); `Ok` says whether what left the profile is exactly that.
    ///
    /// When this session may save and the save fails, the profile is put back as it was and the
    /// raid must not start: it would be a copy (die, crash, and the loadout is still on disk).
    ///
    /// A session that may not save while holding the real stash refuses, with the same kind of
    /// error. The error is the note's line.
    pub fn begin_raid(
        &mut self,
        class: usize,
        told: &Loadout,
        content: &Content,
    ) -> Result<bool, String> {
        // A session that may not save, holding the player's real stash (a newer or unreadable
        // backup keeps the save back), does not raid: nothing of the raid could be kept, the haul
        // would be lost with the session and the loadout copied. They still see their stash.
        if self.keeps_real {
            let why = "Your stash can't be saved this session (see above), so raids are off; fix the file or remove the backup and restart.";
            if self.say(why) {
                warn!("nightfall: {why}");
            }
            return Err(why.into());
        }
        let before = self.profile.clone();
        self.profile.begin_raid(class, content);
        if let Err(e) = self.write() {
            self.profile = before;
            let why = format!("Your stash could not be saved ({e}): the raid did not start.");
            warn!("nightfall: {why}");
            self.note = Some(why.clone());
            return Err(why);
        }
        // What the stash said about its last session is old news once a raid has begun; a
        // standing warning (a file this session may not save over) stays.
        if self.writable {
            self.note = None;
        }
        Ok(self.profile.raid_loadout(class, content) == *told)
    }
}

/// Our outcome, once it came, into the stash and onto the disk at once: a crash after it loses
/// nothing. Only our own outcome ([`Sim::mine`]) is ever settled; the end screen's list of
/// everyone's is for reading.
pub(super) fn settle_run(mut sim: ResMut<Sim>, mut stash: ResMut<Stash>) {
    settle_mine(&mut sim, &mut stash, &content());
}

/// [`settle_run`] with its content in hand. A repeat does nothing: the profile settles a raid
/// only while one is under way.
fn settle_mine(sim: &mut Sim, stash: &mut Stash, content: &Content) {
    let Some(o) = sim.mine.take() else {
        return;
    };
    if stash.profile.settle(&o, content) {
        stash.save();
    }
}

/// Back from a raid without its outcome (we left it, the host went): counted missing.
pub(super) fn after_raid(mut stash: ResMut<Stash>) {
    count_missing(&mut stash, &content());
}

/// [`after_raid`] with its content in hand.
fn count_missing(stash: &mut Stash, content: &Content) {
    if stash.profile.settle_missing(content) {
        stash.note = Some("You left the raid before its end: you were counted missing.".into());
        stash.save();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use nightfall_sim::{HeroOutcome, SavedInventory};

    use super::*;

    /// An empty folder of its own for each test.
    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-stash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn with_gold(gold: u32) -> Profile {
        Profile {
            gold,
            ..Profile::default()
        }
    }

    fn shipped() -> std::sync::Arc<Content> {
        super::super::shipped_content()
    }

    fn stack(item: &str, count: u16) -> SavedStack {
        SavedStack {
            item: item.into(),
            count,
        }
    }

    #[test]
    fn a_missing_file_is_a_fresh_stash() {
        let path = dir("missing").join("stash.ron");
        let l = load(&path);
        assert_eq!(l.profile, Profile::default());
        assert!(l.note.is_none() && l.writable);
    }

    #[test]
    fn saving_keeps_the_last_good_file_as_the_backup() {
        let path = dir("backup").join("stash.ron");
        save(&path, &with_gold(1)).unwrap();
        save(&path, &with_gold(2)).unwrap();
        assert_eq!(load(&path).profile.gold, 2);
        let bak = std::fs::read_to_string(backup_of(&path)).unwrap();
        assert_eq!(Profile::from_ron(&bak).unwrap().gold, 1);
        // A damaged main file never replaces a good backup.
        std::fs::write(&path, "(gold: ").unwrap();
        save(&path, &with_gold(3)).unwrap();
        let bak = std::fs::read_to_string(backup_of(&path)).unwrap();
        assert_eq!(Profile::from_ron(&bak).unwrap().gold, 1);
    }

    #[test]
    fn a_corrupt_file_falls_back_to_the_backup_and_is_kept() {
        let path = dir("corrupt").join("stash.ron");
        save(&path, &with_gold(5)).unwrap();
        save(&path, &with_gold(9)).unwrap();
        // Cut short, as a crash mid-write on a file system without atomic renames would.
        let good = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, &good[..good.len() / 2]).unwrap();
        let l = load(&path);
        assert_eq!(l.profile.gold, 5);
        assert!(l.writable);
        assert!(l.note.unwrap().contains("stash.ron.broken"));
        let kept = std::fs::read_to_string(path.with_extension("ron.broken")).unwrap();
        assert_eq!(kept, &good[..good.len() / 2]);
        // A second break is kept beside the first.
        std::fs::write(&path, "nonsense").unwrap();
        let _ = load(&path);
        assert!(path.with_extension("ron.broken-2").exists());
    }

    #[test]
    fn a_corrupt_file_with_no_backup_is_kept_and_a_new_stash_begins() {
        let path = dir("lone").join("stash.ron");
        std::fs::write(&path, "(gold: \"lots\")").unwrap();
        let l = load(&path);
        assert_eq!(l.profile, Profile::default());
        assert!(l.writable);
        assert!(l.note.unwrap().contains("a new stash begins"));
        assert!(path.with_extension("ron.broken").exists());
    }

    #[test]
    fn a_newer_file_is_never_overwritten() {
        let path = dir("newer").join("stash.ron");
        std::fs::write(&path, "(version: 99, gold: 77)").unwrap();
        let l = load(&path);
        assert!(!l.writable);
        assert!(l.note.as_ref().unwrap().contains("newer"));
        let mut stash = Stash {
            profile: l.profile,
            path: Some(path.clone()),
            writable: l.writable,
            keeps_real: l.real && !l.writable,
            note: l.note,
        };
        stash.profile.gold = 1;
        assert!(!stash.save(), "nothing was written");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "(version: 99, gold: 77)"
        );
        // Not by beginning a raid or by settling one either, and no backup or aside appears.
        // An unwritable session still begins the raid in memory, writing nothing.
        assert!(stash.begin_raid(0, &Loadout::default(), &shipped()).is_ok());
        stash.profile.settle_missing(&shipped());
        stash.save();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "(version: 99, gold: 77)"
        );
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["stash.ron"], "no .bak or .broken beside it");
    }

    #[test]
    fn a_newer_backup_is_refused_like_a_newer_file() {
        let path = dir("newer-bak").join("stash.ron");
        save(&path, &with_gold(3)).unwrap();
        std::fs::write(backup_of(&path), "(version: 99, gold: 8)").unwrap();
        let l = load(&path);
        assert!(!l.writable, "a save would replace the newer backup");
        assert!(l.note.as_ref().unwrap().contains("backup is from a newer"));
        assert_eq!(l.profile.gold, 3, "the stash itself still reads");
        let mut stash = Stash {
            profile: l.profile,
            path: Some(path.clone()),
            writable: l.writable,
            keeps_real: l.real && !l.writable,
            note: l.note,
        };
        stash.profile.gold = 4;
        assert!(!stash.save());
        assert_eq!(
            std::fs::read_to_string(backup_of(&path)).unwrap(),
            "(version: 99, gold: 8)"
        );
        assert_eq!(load(&path).profile.gold, 3, "and the file was not touched");
        // The player still sees their stash, but its raids are off: nothing of one could be kept.
        assert_eq!(stash.profile.gold, 4);
        let before = stash.profile.clone();
        let err = stash
            .begin_raid(0, &Loadout::default(), &shipped())
            .unwrap_err();
        assert!(err.contains("raids are off"), "{err}");
        assert_eq!(stash.profile, before, "nothing left the stash");
        assert!(stash.profile.in_raid.is_none() && stash.profile.raids == 0);
        let note = stash.note.clone().unwrap();
        assert!(note.contains("newer Nightfall") && note.contains("raids are off"));
        let _ = stash.begin_raid(0, &Loadout::default(), &shipped());
        assert_eq!(stash.note.as_deref(), Some(note.as_str()), "said once");
    }

    #[test]
    fn a_damaged_backup_is_kept_aside_before_a_save_can_replace_it() {
        let path = dir("bad-bak").join("stash.ron");
        save(&path, &with_gold(2)).unwrap();
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        let l = load(&path);
        assert!(l.writable);
        assert_eq!(l.profile.gold, 2);
        let note = l.note.as_ref().unwrap();
        assert!(note.contains("stash.ron.bak-broken"), "{note}");
        let kept = path.with_extension("ron.bak-broken");
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "(gold: ");
        // The save that follows replaces the backup, and the copy is still there.
        save(&path, &with_gold(5)).unwrap();
        assert_eq!(
            Profile::from_ron(&std::fs::read_to_string(backup_of(&path)).unwrap())
                .unwrap()
                .gold,
            2
        );
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "(gold: ");
        // A second damaged backup is kept beside the first.
        std::fs::write(backup_of(&path), "nonsense").unwrap();
        let _ = load(&path);
        assert!(path.with_extension("ron.bak-broken-2").exists());
    }

    #[test]
    fn a_damaged_file_and_a_damaged_backup_are_both_kept_and_both_said() {
        let path = dir("both-bad").join("stash.ron");
        std::fs::write(&path, "(gold: \"x\")").unwrap();
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        let l = load(&path);
        assert!(l.writable);
        assert_eq!(l.profile, Profile::default());
        let note = l.note.unwrap();
        assert!(
            note.contains("bak-broken") && note.contains("stash.ron.broken"),
            "{note}"
        );
        assert!(path.with_extension("ron.broken").exists());
        assert!(path.with_extension("ron.bak-broken").exists());
    }

    #[test]
    fn with_no_room_to_keep_a_copy_aside_the_session_does_not_save() {
        let path = dir("full-aside").join("stash.ron");
        std::fs::write(&path, "(gold: ").unwrap();
        // `.broken` and `.broken-2` to `.broken-999` are all there already.
        std::fs::write(path.with_extension("ron.broken"), "first").unwrap();
        for n in 2..=MAX_ASIDE {
            std::fs::write(path.with_extension(format!("ron.broken-{n}")), "kept").unwrap();
        }
        assert_eq!(aside_of(&path, "broken"), None);
        let l = load(&path);
        assert!(!l.writable);
        assert!(l.note.unwrap().contains("could not be set aside"));
        // Nothing was reused: the first copy is as it was, and so is the damaged file.
        assert_eq!(
            std::fs::read_to_string(path.with_extension("ron.broken")).unwrap(),
            "first"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "(gold: ");
    }

    #[test]
    fn a_raid_whose_begin_cannot_be_saved_does_not_start() {
        let content = shipped();
        // A file where the `nightfall` folder should be: nothing can be written under it.
        let root = dir("unwritable");
        let blocker = root.join("nightfall");
        std::fs::write(&blocker, "not a folder").unwrap();
        let mut stash = Stash {
            profile: Profile::default(),
            path: Some(blocker.join("stash.ron")),
            writable: true,
            keeps_real: false,
            note: Some("old news".into()),
        };
        stash.profile.stash_add(stack("ruby_shard", 3), &content);
        stash.profile.loadout = SavedInventory {
            pack: vec![stack("tough_bread", 2)],
            ..SavedInventory::default()
        };
        let before = stash.profile.clone();
        let brought = told(&stash, 0, &content);
        let err = stash.begin_raid(0, &brought, &content).unwrap_err();
        assert!(err.contains("the raid did not start"), "{err}");
        assert_eq!(stash.profile, before, "the loadout is where it was");
        assert!(stash.profile.in_raid.is_none() && stash.profile.raids == 0);
        assert_eq!(
            stash.note.as_deref(),
            Some(err.as_str()),
            "and the menu says why"
        );
        assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "not a folder");
    }

    #[test]
    fn a_unit_test_never_opens_a_real_stash_even_with_a_home_set() {
        use crate::local_state::test_env::{EnvGuard, ENV_LOCK};
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = dir("home");
        std::fs::create_dir_all(root.join("nightfall")).unwrap();
        let file = root.join("nightfall").join("stash.ron");
        // A stash that a real open would settle (a raid under way) and rewrite.
        std::fs::write(
            &file,
            "(gold: 7, in_raid: Some((map: \"duskwood\", brought: ())))",
        )
        .unwrap();
        let _home = EnvGuard::set("BENILLA_HOME", root.to_str().unwrap());
        let _give = EnvGuard::set("NIGHTFALL_GIVE", "ruby_shard:3");
        super::super::prepare(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).unwrap();
        let mut stash = Stash::open();
        assert_eq!(stash.profile, Profile::default(), "a fresh stash in memory");
        assert!(!stash.save(), "which writes nothing");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "(gold: 7, in_raid: Some((map: \"duskwood\", brought: ())))"
        );
        let names = std::fs::read_dir(root.join("nightfall")).unwrap().count();
        assert_eq!(names, 1, "no backup or aside appeared");
    }

    #[test]
    fn a_read_only_main_file_leaves_a_damaged_backup_alone() {
        // Already read-only for the main file: copying the backup aside at every launch would pile
        // `bak-broken-N` files up for nothing.
        for main in ["(version: 99, gold: 77)"] {
            let path = dir("ro-main").join("stash.ron");
            std::fs::write(&path, main).unwrap();
            std::fs::write(backup_of(&path), "(gold: ").unwrap();
            for _ in 0..3 {
                let l = load(&path);
                assert!(!l.writable);
                assert!(l.note.unwrap().contains("newer"));
            }
            assert!(!path.with_extension("ron.bak-broken").exists());
            assert_eq!(
                std::fs::read_to_string(backup_of(&path)).unwrap(),
                "(gold: "
            );
        }
        // An unreadable main file (a folder in its place) likewise.
        let path = dir("ro-main-dir").join("stash.ron");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        assert!(!load(&path).writable);
        assert!(!path.with_extension("ron.bak-broken").exists());
    }

    #[test]
    fn a_missing_file_and_a_damaged_backup_say_a_new_stash_begins() {
        let path = dir("missing-bad-bak").join("stash.ron");
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        let l = load(&path);
        assert!(l.writable);
        assert_eq!(l.profile, Profile::default());
        let note = l.note.unwrap();
        assert!(note.contains("bak-broken"), "{note}");
        assert!(note.contains("a new, empty stash begins"), "{note}");
        // A first run (no files at all) says nothing.
        let none = load(&dir("first-run").join("stash.ron"));
        assert!(none.note.is_none());
        // And a missing file restored from a good backup says that, not a new stash.
        let path = dir("missing-good-bak").join("stash.ron");
        save(&path, &with_gold(1)).unwrap();
        save(&path, &with_gold(2)).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!load(&path).note.unwrap().contains("new, empty"));
    }

    #[test]
    fn a_damaged_backup_is_kept_aside_once_however_often_the_stash_is_opened() {
        let aside = |path: &Path| {
            std::fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter(|e| {
                    e.as_ref()
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .contains("bak-broken")
                })
                .count()
        };
        // With a good stash file: the backup becomes that file's text.
        let path = dir("once-main").join("stash.ron");
        save(&path, &with_gold(2)).unwrap();
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        for _ in 0..3 {
            // Opened, and never saved.
            assert!(load(&path).writable);
        }
        assert_eq!(aside(&path), 1, "one copy, not one a launch");
        assert_eq!(
            std::fs::read_to_string(backup_of(&path)).unwrap(),
            std::fs::read_to_string(&path).unwrap(),
            "the backup is the stash file now"
        );
        assert_eq!(
            std::fs::read_to_string(path.with_extension("ron.bak-broken")).unwrap(),
            "(gold: "
        );
        assert_eq!(load(&path).profile.gold, 2);
        // With no stash file: the backup is removed once it is safe aside.
        let path = dir("once-none").join("stash.ron");
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        for _ in 0..3 {
            assert!(load(&path).writable);
        }
        assert_eq!(aside(&path), 1);
        assert!(!backup_of(&path).exists());
        // With a damaged stash file as well: the backup is kept aside once and removed.
        let path = dir("once-both").join("stash.ron");
        std::fs::write(&path, "(gold: \"x\")").unwrap();
        std::fs::write(backup_of(&path), "(gold: ").unwrap();
        for _ in 0..3 {
            let _ = load(&path);
        }
        assert_eq!(aside(&path), 1);
    }

    #[test]
    fn a_damaged_stash_file_is_kept_aside_once_however_often_the_stash_is_opened() {
        let broken = |path: &Path| {
            std::fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter(|e| {
                    let name = e.as_ref().unwrap().file_name();
                    let name = name.to_string_lossy();
                    name.contains(".broken") && !name.contains("bak-broken")
                })
                .count()
        };
        // A good backup restores the stash, and the stash file is rewritten from it.
        let path = dir("once-restored").join("stash.ron");
        save(&path, &with_gold(5)).unwrap();
        save(&path, &with_gold(9)).unwrap();
        std::fs::write(&path, "(gold: ").unwrap();
        for _ in 0..3 {
            let l = load(&path);
            assert!(l.writable);
            assert_eq!(l.profile.gold, 5);
        }
        assert_eq!(broken(&path), 1, "one copy, not one a launch");
        assert_eq!(
            std::fs::read_to_string(path.with_extension("ron.broken")).unwrap(),
            "(gold: "
        );
        assert_eq!(
            Profile::from_ron(&std::fs::read_to_string(&path).unwrap())
                .unwrap()
                .gold,
            5
        );
        // No backup: the damaged file is removed, and the fresh stash saves normally.
        let path = dir("once-removed").join("stash.ron");
        std::fs::write(&path, "(gold: ").unwrap();
        for _ in 0..3 {
            let l = load(&path);
            assert!(l.writable);
            assert_eq!(l.profile, Profile::default());
        }
        assert_eq!(broken(&path), 1);
        assert!(!path.exists());
        save(&path, &with_gold(2)).unwrap();
        assert_eq!(load(&path).profile.gold, 2);
        // A read-only session leaves every file alone: a newer backup beside a damaged file.
        let path = dir("once-readonly").join("stash.ron");
        std::fs::write(&path, "(gold: ").unwrap();
        std::fs::write(backup_of(&path), "(version: 99, gold: 8)").unwrap();
        for _ in 0..3 {
            assert!(!load(&path).writable);
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "(gold: ");
        assert_eq!(
            std::fs::read_to_string(backup_of(&path)).unwrap(),
            "(version: 99, gold: 8)"
        );
    }

    #[test]
    fn a_lost_file_comes_back_from_its_backup() {
        let path = dir("lost").join("stash.ron");
        save(&path, &with_gold(4)).unwrap();
        save(&path, &with_gold(6)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let l = load(&path);
        assert_eq!(l.profile.gold, 4);
        assert!(l.note.unwrap().contains("backup"));
    }

    #[test]
    fn a_file_that_cannot_be_read_is_left_alone_and_the_session_does_not_save() {
        // A folder where the file should be: reading it is an error other than "not found".
        let path = dir("unreadable").join("stash.ron");
        std::fs::create_dir(&path).unwrap();
        let l = load(&path);
        assert!(!l.writable);
        assert!(l.note.unwrap().contains("does not save"));
        assert!(path.is_dir(), "left as it was");
    }

    #[test]
    fn every_save_is_whole_or_not_at_all() {
        // The write goes through a temporary file and a rename: no half-written stash.ron, and
        // the temporary is gone after it.
        let path = dir("atomic").join("stash.ron");
        save(&path, &with_gold(1)).unwrap();
        save(&path, &with_gold(2)).unwrap();
        assert!(Profile::from_ron(&std::fs::read_to_string(&path).unwrap()).is_ok());
        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), "{tmp:?} was renamed away");
    }

    /// What `class`'s hero would be told to bring now ([`super::session::own_loadout`], which reads
    /// the global content a unit test has not loaded).
    fn told(stash: &Stash, class: usize, content: &Content) -> Loadout {
        stash.profile.raid_loadout(class, content)
    }

    #[test]
    fn a_raid_that_never_ended_is_counted_missing_at_the_next_start_once() {
        let content = shipped();
        let path = dir("crash").join("stash.ron");
        // A session: a stash with things, a loadout, a raid begun (saved before it starts).
        let mut session = Stash::open_at(Some(path.clone()), &content);
        session.profile.stash_add(stack("ruby_shard", 3), &content);
        session.profile.loadout = SavedInventory {
            pack: vec![stack("tough_bread", 2)],
            ..SavedInventory::default()
        };
        let brought = told(&session, 0, &content);
        assert_eq!(session.begin_raid(0, &brought, &content), Ok(true));
        let at_start = Profile::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            at_start.in_raid.is_some() && at_start.raids == 1,
            "the raid is on disk before it starts"
        );
        // The game crashes here: nothing more is saved. The next start finds the raid.
        drop(session);
        let mut expected = at_start.clone();
        assert!(expected.settle_missing(&content));
        let next = Stash::open_at(Some(path.clone()), &content);
        assert_eq!(next.profile, expected, "settled exactly as a missing raid");
        assert!(next.profile.in_raid.is_none());
        assert!(next.note.as_ref().unwrap().contains("missing"));
        assert_eq!(next.profile.raids, 1, "and the raid is counted once");
        let settled = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            Profile::from_ron(&settled).unwrap(),
            expected,
            "and that is on disk"
        );
        // Another start, and the menu's own check, settle nothing twice.
        drop(next);
        let mut again = Stash::open_at(Some(path.clone()), &content);
        assert_eq!(again.profile, expected);
        assert!(again.note.is_none());
        count_missing(&mut again, &content);
        assert_eq!(again.profile, expected);
        assert!(again.note.is_none(), "there was nothing to count");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), settled);
    }

    #[test]
    fn leaving_a_raid_without_its_outcome_counts_it_missing_once() {
        let content = shipped();
        let path = dir("left").join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), &content);
        let brought = told(&stash, 0, &content);
        assert_eq!(stash.begin_raid(0, &brought, &content), Ok(true));
        count_missing(&mut stash, &content);
        assert!(stash.profile.in_raid.is_none());
        assert!(stash.note.as_ref().unwrap().contains("missing"));
        let saved = Profile::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved, stash.profile, "and it is on disk");
        stash.note = None;
        count_missing(&mut stash, &content);
        assert!(stash.note.is_none(), "the second leave counts nothing");
    }

    fn extracted_with(carried: nightfall_sim::Inventory) -> HeroOutcome {
        HeroOutcome {
            player: 0,
            name: "Us".into(),
            fate: nightfall_sim::Fate::Extracted {
                exit: "The bridge".into(),
            },
            time: 600.0,
            kills: 0,
            carried,
            overflow: Vec::new(),
            recovered: Vec::new(),
            recovery: None,
        }
    }

    #[test]
    fn a_raid_that_ended_is_settled_once_from_our_outcome_and_saved_at_once() {
        let content = shipped();
        let path = dir("settle").join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), &content);
        let brought = told(&stash, 0, &content);
        assert_eq!(stash.begin_raid(0, &brought, &content), Ok(true));
        // Our hero brings out a ruby shard on top of what they went in with.
        let ruby = content.item("ruby_shard").unwrap();
        let mut carried = brought.inventory.clone();
        assert!(carried
            .add(nightfall_sim::Stack::new(ruby, 1), &content)
            .is_none());
        let mut sim = Sim::for_test(nightfall_sim::World::new(content.clone()));
        sim.mine = Some(extracted_with(carried));
        // Everyone's outcomes are for the end screen: settling never reads them.
        sim.over = Some(vec![extracted_with(brought.inventory.clone())]);

        settle_mine(&mut sim, &mut stash, &content);
        assert!(sim.mine.is_none(), "taken");
        assert!(sim.over.is_some(), "the end screen's list is left alone");
        let on_disk = Profile::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(on_disk.in_raid.is_none() && on_disk.extracted == 1);
        assert_eq!(on_disk, stash.profile, "saved at once");
        let had = on_disk.loadout.clone();

        // The same outcome again settles nothing: no second haul.
        sim.mine = Some(extracted_with(brought.inventory.clone()));
        settle_mine(&mut sim, &mut stash, &content);
        assert_eq!(stash.profile.loadout, had);
        assert_eq!(stash.profile.extracted, 1);
        // And with no outcome (the end screen's list only) nothing happens.
        settle_mine(&mut sim, &mut stash, &content);
        assert_eq!(stash.profile, on_disk);
    }

    #[test]
    fn the_raid_commits_exactly_the_loadout_it_was_told() {
        // The lobby race's invariant: whatever pick a player sent (their profile's raid loadout
        // then), beginning the raid takes exactly that from the profile.
        let content = shipped();
        for loadout in [
            SavedInventory::default(),
            SavedInventory {
                pack: vec![stack("tough_bread", 2), stack("ruby_shard", 1)],
                ..SavedInventory::default()
            },
        ] {
            for class in 0..content.classes.len() {
                let mut stash = Stash::open_at(None, &content);
                stash.profile.loadout = loadout.clone();
                let brought = told(&stash, class, &content);
                assert_eq!(
                    stash.begin_raid(class, &brought, &content),
                    Ok(true),
                    "class {class}, loadout {loadout:?}"
                );
            }
        }
        // A pick that has gone stale (the stash changed after it was sent) is not committed.
        let mut stash = Stash::open_at(None, &content);
        let brought = told(&stash, 0, &content);
        stash.profile.loadout = SavedInventory {
            pack: vec![stack("ruby_shard", 1)],
            ..SavedInventory::default()
        };
        assert_eq!(stash.begin_raid(0, &brought, &content), Ok(false));
    }

    #[test]
    fn a_stash_with_nowhere_to_save_is_still_a_stash() {
        let content = shipped();
        let mut stash = Stash::open_at(None, &content);
        stash.profile.gold = 3;
        assert!(!stash.save());
        assert!(stash.note.is_none(), "no state folder is not a failure");
    }

    #[test]
    fn nightfall_give_ignores_bad_entries_and_says_so() {
        let content = shipped();
        let (stacks, warnings) = parse_give(
            "ruby_shard:3, hooded_lantern:1,nonsense,ruby_shard:-1,ruby_shard:0,ruby_shard:99999,no_such_item:2,:4,gold_ring:x,,",
            &content,
        );
        assert_eq!(
            stacks,
            [stack("ruby_shard", 3), stack("hooded_lantern", 1)],
            "only the good ones"
        );
        assert_eq!(warnings.len(), 7, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("nonsense")));
        assert!(warnings.iter().any(|w| w.contains("no_such_item")));
        let (none, quiet) = parse_give("", &content);
        assert!(none.is_empty() && quiet.is_empty());
    }
}
