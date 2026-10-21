//! The WotLK archives, read in the client's own order: a later archive wins. Only the archives a
//! stock 3.3.5a install has are mounted, so the port is the same from any 3.3.5 client; a private
//! server's own `patch-?.MPQ` files are left out.

use std::path::Path;

use anyhow::{bail, Context, Result};
use wow_mpq::Archive;

/// A stock 3.3.5a `Data` folder's archives, ascending priority.
const ORDER: &[&str] = &[
    "common.MPQ",
    "common-2.MPQ",
    "expansion.MPQ",
    "lichking.MPQ",
    "enUS/locale-enUS.MPQ",
    "enUS/expansion-locale-enUS.MPQ",
    "enUS/lichking-locale-enUS.MPQ",
    "enUS/speech-enUS.MPQ",
    "enUS/expansion-speech-enUS.MPQ",
    "enUS/lichking-speech-enUS.MPQ",
    "patch.MPQ",
    "enUS/patch-enUS.MPQ",
    "patch-2.MPQ",
    "enUS/patch-enUS-2.MPQ",
    "patch-3.MPQ",
    "enUS/patch-enUS-3.MPQ",
];

pub struct Wotlk {
    /// Highest priority first.
    archives: Vec<Archive>,
}

impl Wotlk {
    pub fn open(data: &Path) -> Result<Self> {
        let mut archives = Vec::new();
        for name in ORDER.iter().rev() {
            let path = data.join(name);
            if path.exists() {
                archives.push(Archive::open(&path).with_context(|| format!("open {}", path.display()))?);
            }
        }
        if archives.is_empty() {
            bail!("no WotLK archives in {}", data.display());
        }
        Ok(Self { archives })
    }

    /// The highest-priority copy of `name`.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        self.try_read(name)?.with_context(|| format!("{name} is in no WotLK archive"))
    }

    /// The highest-priority copy of `name`, or `None` where no archive has it.
    pub fn try_read(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        for a in &mut self.archives {
            if a.find_file(name)?.is_some() {
                return a.read_file(name).map(Some).with_context(|| format!("read {name}"));
            }
        }
        Ok(None)
    }
}