//! The player's things between raids, on disk: `benilla-config/nightfall/stash.ron`, a
//! [`Profile`] in RON (spec §8.3: each player's stash on their own machine). Never lost silently:
//! every save keeps the last good file as `stash.ron.bak`; a file that does not read is kept as
//! `stash.ron.broken` (numbered when there is one already) and the backup tried; a file from a
//! newer Nightfall is left untouched, and this session does not save over it. Whatever happened
//! is said on the menu ([`Stash::note`]).
//!
//! A raid is written to disk before it starts ([`Stash::begin_raid`]): the loadout leaves the
//! profile for `in_raid`. A raid that never ends (a crash, a lost connection, the host going) is
//! found there at the next start and settled as missing, once ([`Stash::open`], [`after_raid`]);
//! a raid that ends is settled by its outcome ([`settle_run`]) and saved at once.

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
}

fn backup_of(path: &Path) -> PathBuf {
    path.with_extension("ron.bak")
}

/// Where a broken file is set aside: `stash.ron.broken`, or the first `stash.ron.broken-N` free.
fn broken_of(path: &Path) -> PathBuf {
    let free = |p: &Path| matches!(crate::local_state::read_to_string(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
    let first = path.with_extension("ron.broken");
    if free(&first) {
        return first;
    }
    (2..1000)
        .map(|n| path.with_extension(format!("ron.broken-{n}")))
        .find(|p| free(p))
        .unwrap_or(first)
}

/// The backup's profile, if it reads.
fn from_backup(path: &Path) -> Option<Profile> {
    let text = crate::local_state::read_to_string(&backup_of(path)).ok()?;
    Profile::from_ron(&text).ok()
}

/// The stash at `path`.
pub(super) fn load(path: &Path) -> Loaded {
    let fresh = |note: Option<String>, writable: bool| Loaded {
        profile: Profile::default(),
        note,
        writable,
    };
    let text = match crate::local_state::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return match from_backup(path) {
                Some(profile) => Loaded {
                    profile,
                    note: Some("Your stash file was missing; it came back from its backup.".into()),
                    writable: true,
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
        },
        Err(ProfileError::Newer { .. }) => fresh(
            Some(
                "Your stash is from a newer Nightfall: it is left untouched, and this session's raids are not saved."
                    .into(),
            ),
            false,
        ),
        Err(ProfileError::Parse(why)) => {
            let aside = broken_of(path);
            if crate::local_state::write_atomic(&aside, &text).is_err() {
                return fresh(
                    Some(format!(
                        "Your stash file is damaged ({why}) and could not be set aside: it is left as it is, and this session does not save."
                    )),
                    false,
                );
            }
            let kept = aside
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match from_backup(path) {
                Some(profile) => Loaded {
                    profile,
                    note: Some(format!(
                        "Your stash file was damaged ({why}): it was kept as {kept}, and your stash came back from its backup."
                    )),
                    writable: true,
                },
                None => fresh(
                    Some(format!(
                        "Your stash file was damaged ({why}) and had no good backup: it was kept as {kept}, and a new stash begins."
                    )),
                    true,
                ),
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
    /// What the menu says about the stash: a file that did not read, a raid counted missing, a
    /// move refused.
    pub note: Option<String>,
}

impl Stash {
    /// The stash from `benilla-config/nightfall/stash.ron`; a raid that never ended settled as
    /// missing; and `NIGHTFALL_GIVE`'s things added (a test hook). With no state folder (a unit
    /// test, a capture) there is nothing to read and nothing is saved.
    pub fn open() -> Self {
        let path = crate::local_state::home().map(|h| h.join("nightfall").join("stash.ron"));
        let content = content();
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
            },
        };
        let mut stash = Stash {
            profile: loaded.profile,
            path,
            writable: loaded.writable,
            note: loaded.note,
        };
        if stash.profile.settle_missing(content) {
            stash.note = Some("Your last raid ended without you: you were counted missing.".into());
            stash.save();
        }
        stash
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
    /// The error is the note's line.
    pub fn begin_raid(
        &mut self,
        class: usize,
        told: &Loadout,
        content: &Content,
    ) -> Result<bool, String> {
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
