//! The machine's wish list of WotLK art: `WoW\wotlk-port.txt` in the main checkout, beside the
//! `WoW\Data` it builds `patch-W.MPQ` into. Art is machine-local (Blizzard data never goes into
//! git), so every session and branch on this machine shares one list and one archive: a session
//! adds what it needs and rebuilds, and nothing another session added is lost.
//!
//! One entry per line, `#` starts a comment:
//!
//! ```text
//! wotlk D:\Program Files\...\Data        the WotLK client's Data folder
//! race 10                                Blood Elf (10) or Draenei (11) heroes
//! fill Azeroth                           every model and WMO the map's 1.12 tiles name but lack
//! wmo World\wmo\...\Stormwind.wmo        one WMO, its groups, textures and doodads
//! model Creature\Foo\Foo                 one M2 (no extension)
//! creature 12345                         a WotLK CreatureDisplayInfo row and what it names
//! file Sound\Music\...\Foo.mp3           any file, copied as is
//! tiles Azeroth 31-35 38-42              WotLK terrain tiles the 1.12 data lacks (x range, y range)
//! zone Azeroth Dun Morogh                every tile a WotLK zone (and its subzones) covers
//! ```
//!
//! `tiles` and `zone` are newer than the other kinds: a together-port built before them rejects
//! the whole list, so they are usable on the shared list once every branch has merged them.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// One thing to port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Race(u32),
    Fill(String),
    Wmo(String),
    Model(String),
    Creature(u32),
    File(String),
    /// A map's tiles `x0..=x1` by `y0..=y1`.
    Tiles { map: String, x: (u32, u32), y: (u32, u32) },
    /// A map's tiles that a WotLK AreaTable zone covers, by its name.
    Zone { map: String, name: String },
}

/// `a-b` or `a`, each 0..64.
fn range(s: &str) -> Result<(u32, u32)> {
    let (a, b) = s.split_once('-').unwrap_or((s, s));
    let (a, b): (u32, u32) = (a.trim().parse()?, b.trim().parse()?);
    if a > b || b > 63 {
        bail!("tile range `{s}`: want a-b with a <= b <= 63");
    }
    Ok((a, b))
}

impl Entry {
    pub fn parse(kind: &str, arg: &str) -> Result<Self> {
        let arg = arg.trim();
        if arg.is_empty() {
            bail!("`{kind}` needs an argument");
        }
        Ok(match kind {
            "race" => match arg.parse()? {
                r @ (10 | 11) => Entry::Race(r),
                r => bail!("race {r}: only Blood Elf (10) and Draenei (11) are portable"),
            },
            "fill" => Entry::Fill(arg.to_owned()),
            "wmo" => Entry::Wmo(arg.to_owned()),
            "model" => Entry::Model(strip_m2(arg).to_owned()),
            "creature" => Entry::Creature(arg.parse()?),
            "file" => Entry::File(arg.to_owned()),
            "tiles" => match arg.split_whitespace().collect::<Vec<_>>()[..] {
                [map, x, y] => Entry::Tiles { map: map.to_owned(), x: range(x)?, y: range(y)? },
                _ => bail!("`tiles <Map> <x0-x1> <y0-y1>`"),
            },
            "zone" => match arg.split_once(char::is_whitespace) {
                Some((map, name)) if !name.trim().is_empty() => {
                    Entry::Zone { map: map.to_owned(), name: name.split_whitespace().collect::<Vec<_>>().join(" ") }
                }
                _ => bail!("`zone <Map> <AreaTable name>`"),
            },
            other => bail!("unknown kind `{other}`: race, fill, wmo, model, creature, file, tiles or zone"),
        })
    }

    pub fn line(&self) -> String {
        match self {
            Entry::Race(r) => format!("race {r}"),
            Entry::Fill(m) => format!("fill {m}"),
            Entry::Wmo(p) => format!("wmo {p}"),
            Entry::Model(p) => format!("model {p}"),
            Entry::Creature(id) => format!("creature {id}"),
            Entry::File(p) => format!("file {p}"),
            Entry::Tiles { map, x, y } => format!("tiles {map} {}-{} {}-{}", x.0, x.1, y.0, y.1),
            Entry::Zone { map, name } => format!("zone {map} {name}"),
        }
    }
}

/// A model path without its `.m2`, `.mdx` or `.mdl` (old WMO doodad lists still say `.MDL`).
pub fn strip_m2(path: &str) -> &str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".m2") {
        &path[..path.len() - 3]
    } else if lower.ends_with(".mdx") || lower.ends_with(".mdl") {
        &path[..path.len() - 4]
    } else {
        path
    }
}

pub struct Manifest {
    pub path: PathBuf,
    /// The file's lines as written, comments included.
    lines: Vec<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::from(
                "# together-port's wish list for this machine; see docs/WOTLK-PORT.md.\n\
                 # `together-port add <kind> <arg>` adds a line and rebuilds WoW\\Data\\patch-W.MPQ.\n",
            ),
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        Ok(Self { path: path.to_owned(), lines: text.lines().map(str::to_owned).collect() })
    }

    /// The `(kind, argument)` of every non-comment line.
    fn items(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines.iter().filter_map(|l| {
            let l = l.split('#').next().unwrap_or("").trim();
            if l.is_empty() {
                return None;
            }
            Some(l.split_once(char::is_whitespace).unwrap_or((l, "")))
        })
    }

    /// The WotLK `Data` folder: the `wotlk` line, else `$WOTLK_DATA`.
    pub fn wotlk(&self) -> Option<PathBuf> {
        self.items()
            .find(|(k, _)| *k == "wotlk")
            .map(|(_, v)| PathBuf::from(v.trim()))
            .or_else(|| std::env::var_os("WOTLK_DATA").map(PathBuf::from))
    }

    pub fn entries(&self) -> Result<Vec<Entry>> {
        let mut out: Vec<Entry> = Vec::new();
        for (k, v) in self.items().filter(|(k, _)| *k != "wotlk") {
            let e = Entry::parse(k, v).with_context(|| format!("{}: `{k} {v}`", self.path.display()))?;
            if !out.contains(&e) {
                out.push(e);
            }
        }
        Ok(out)
    }

    /// Add a line unless an equal one is there; true if it was added.
    pub fn add(&mut self, line: String) -> bool {
        let new_entry = line.split_once(' ').and_then(|(k, v)| Entry::parse(k, v).ok());
        let present = self.items().any(|(k, v)| {
            if k == "wotlk" {
                line.starts_with("wotlk ")
            } else {
                new_entry.is_some() && Entry::parse(k, v).ok() == new_entry
            }
        });
        if present && !line.starts_with("wotlk ") {
            return false;
        }
        if line.starts_with("wotlk ") {
            self.lines.retain(|l| !l.trim_start().starts_with("wotlk "));
        }
        self.lines.push(line);
        true
    }

    pub fn save(&self) -> Result<()> {
        let mut text = self.lines.join("\n");
        text.push('\n');
        std::fs::write(&self.path, text).with_context(|| format!("write {}", self.path.display()))
    }
}

/// Where the port lives: the main checkout's `WoW` folder, also from a worktree under its
/// `.claude\worktrees\`, so every session on the machine shares it. `$TOGETHER_PORT_ROOT` names
/// the checkout outright.
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn find() -> Result<Self> {
        if let Some(root) = std::env::var_os("TOGETHER_PORT_ROOT") {
            return Ok(Self { root: root.into() });
        }
        let cwd = std::env::current_dir()?;
        for dir in cwd.ancestors() {
            if dir.file_name().is_some_and(|n| n == "worktrees")
                && dir.parent().and_then(Path::file_name).is_some_and(|n| n == ".claude")
            {
                let root = dir.parent().and_then(Path::parent).context("a worktree folder without a checkout")?;
                return Ok(Self { root: root.to_owned() });
            }
        }
        cwd.ancestors()
            .find(|d| d.join("crates").join("together-port").is_dir())
            .map(|d| Self { root: d.to_owned() })
            .context("run from inside the azeroth-together checkout (or set TOGETHER_PORT_ROOT)")
    }

    pub fn manifest(&self) -> PathBuf {
        self.root.join("WoW").join("wotlk-port.txt")
    }
    pub fn vanilla(&self) -> PathBuf {
        self.root.join("WoW").join("Data")
    }
    pub fn archive(&self) -> PathBuf {
        self.vanilla().join("patch-W.MPQ")
    }
    pub fn lock(&self) -> PathBuf {
        self.root.join("WoW").join("wotlk-port.lock")
    }
    pub fn ids(&self) -> PathBuf {
        self.root.join("WoW").join("wotlk-port-ids.txt")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_parse_dedupe_and_skip_comments() {
        let mut m = Manifest { path: "x".into(), lines: vec!["# hi".into(), "wotlk C:\\Data".into(), "race 10 # elves".into()] };
        assert!(m.add("model Creature\\Foo\\Foo.m2".into()));
        assert!(!m.add("model Creature\\Foo\\Foo".into()), "the same model");
        assert!(!m.add("race 10".into()));
        assert_eq!(m.wotlk(), Some(PathBuf::from("C:\\Data")));
        assert_eq!(m.entries().unwrap(), vec![Entry::Race(10), Entry::Model("Creature\\Foo\\Foo".into())]);
        assert!(Entry::parse("race", "4").is_err());
        assert!(Entry::parse("castle", "x").is_err());
        let t = Entry::parse("tiles", "Azeroth 31-35 40").unwrap();
        assert_eq!(t, Entry::Tiles { map: "Azeroth".into(), x: (31, 35), y: (40, 40) });
        assert_eq!(t.line(), "tiles Azeroth 31-35 40-40");
        assert!(Entry::parse("tiles", "Azeroth 35-31 40").is_err());
        assert!(Entry::parse("tiles", "Azeroth 31-64 40").is_err());
        let z = Entry::parse("zone", "Azeroth  Dun   Morogh").unwrap();
        assert_eq!(z.line(), "zone Azeroth Dun Morogh");
        assert!(Entry::parse("zone", "Azeroth").is_err());
    }
}