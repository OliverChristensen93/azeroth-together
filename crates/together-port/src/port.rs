//! Porting WotLK art into 1.12's formats: what each [`crate::manifest::Entry`] brings. The
//! Blood Elf (race 10) and Draenei (race 11) heroes (bodies, skins, hair, voices and the rows that
//! name them), single models, creatures, WMOs, files, and `fill`: every model and WMO a map's
//! 1.12 tiles name but the trimmed 1.12 data lacks, and `tiles`/`zone`: WotLK terrain tiles the
//! 1.12 data lacks, converted ([`adt`]), with everything they name.
//!
//! A 1.12 table is read from the vanilla chain and extended with the ported rows; the whole table
//! goes into the output archive, which sits above the vanilla archives in the patch chain. A
//! WotLK id the 1.12 table already uses for something else is given a fresh id.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{bail, Context, Result};
use benilla_formats::Chain;

use crate::adt::{self, Liquid};
use crate::blp;
use crate::dbc::{Dbc, F};
use crate::m2;
use crate::mpq_out::MpqOut;
use crate::source::Wotlk;

/// The races ported, with the 1.12 FactionTemplate each plays under (WotLK's 1610 and 1629 do
/// not exist in 1.12): Blood Elf as an Orc (Horde), Draenei as a Human (Alliance).
pub const RACES: &[(u32, u32)] = &[(10, 2), (11, 1)];
/// WotLK's Death Knight, which 1.12 has no class for.
const DEATH_KNIGHT: u8 = 6;
/// The 1.12 model whose PlayableAnimationLookup every ported model's is built from.
pub const PLAYABLE_TEMPLATE: &str = "Character\\Human\\Male\\HumanMale.m2";
/// M2 event tags whose payload is a SoundEntries id.
const SOUND_EVENTS: &[&[u8; 4]] = &[b"$SND", b"$DSO", b"$DSL", b"$DSE", b"$CSD"];

pub struct Port<'a> {
    wotlk: &'a mut Wotlk,
    vanilla: &'a Chain,
    pub out: MpqOut,
    /// 1.12 tables being extended, by name.
    tables: HashMap<String, Dbc>,
    /// WotLK tables, by name.
    wtables: HashMap<String, Dbc>,
    /// WotLK SoundEntries id to the 1.12 id it was ported as.
    sounds: HashMap<u32, u32>,
    template: Vec<m2::Playable>,
    /// Files the WotLK archives lacked.
    pub missing: Vec<String>,
    /// Lights, ribbons and particle emitters left out of the converted models.
    pub dropped: (usize, usize, usize),
    /// What a session needs to know about the ids it got (`creature` entries).
    pub ids: Vec<String>,
}

/// The chunks of a WoW chunked file (ADT, WMO): each tag as read (`MVER`, not `REVM`) and its
/// data.
fn chunks(b: &[u8]) -> Vec<(String, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 8 <= b.len() {
        let tag: String = b[i..i + 4].iter().rev().map(|&c| c as char).collect();
        let n = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let end = (i + 8 + n).min(b.len());
        out.push((tag, &b[i + 8..end]));
        i = end;
    }
    out
}

/// The NUL-separated names of an `MMDX`, `MWMO`, `MTEX`, `MOTX` or `MODN` chunk.
fn names(data: &[u8]) -> Vec<String> {
    data.split(|&c| c == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect()
}

fn wdt_path(map: &str) -> String {
    format!("World\\Maps\\{map}\\{map}.wdt")
}

fn tile_path(map: &str, x: u32, y: u32) -> String {
    format!("World\\Maps\\{map}\\{map}_{x}_{y}.adt")
}

fn dbc_path(name: &str) -> String {
    format!("DBFilesClient\\{name}.dbc")
}

impl<'a> Port<'a> {
    pub fn new(wotlk: &'a mut Wotlk, vanilla: &'a Chain) -> Result<Self> {
        let template = m2::read_playable(&vanilla.read(PLAYABLE_TEMPLATE)?)?;
        Ok(Self {
            wotlk,
            vanilla,
            out: MpqOut::default(),
            tables: HashMap::new(),
            wtables: HashMap::new(),
            sounds: HashMap::new(),
            template,
            missing: Vec::new(),
            dropped: (0, 0, 0),
            ids: Vec::new(),
        })
    }

    /// A 1.12 table, loaded on first use.
    fn v(&mut self, name: &str) -> Result<&mut Dbc> {
        if !self.tables.contains_key(name) {
            let t = Dbc::parse(&self.vanilla.read(&dbc_path(name))?).with_context(|| format!("1.12 {name}"))?;
            self.tables.insert(name.to_owned(), t);
        }
        Ok(self.tables.get_mut(name).unwrap())
    }

    /// A WotLK table, loaded on first use.
    fn w(&mut self, name: &str) -> Result<&Dbc> {
        if !self.wtables.contains_key(name) {
            let t = Dbc::parse(&self.wotlk.read(&dbc_path(name))?).with_context(|| format!("WotLK {name}"))?;
            self.wtables.insert(name.to_owned(), t);
        }
        Ok(&self.wtables[name])
    }

    /// The WotLK rows of `table` whose `col` is `val`.
    fn w_rows(&mut self, table: &str, col: usize, val: u32) -> Result<Vec<Vec<u8>>> {
        Ok(self.w(table)?.rows.iter().filter(|r| Dbc::u(r, col) == val).cloned().collect())
    }

    /// `id` if the 1.12 table does not use it yet, else a fresh one.
    fn free_id(&mut self, table: &str, id: u32) -> Result<u32> {
        let t = self.v(table)?;
        Ok(if id != 0 && t.find(id).is_none() { id } else { t.max_id() + 1 })
    }

    /// Copy a WotLK file into the output as is, unless 1.12 has it.
    fn copy(&mut self, path: &str) -> Result<()> {
        if path.is_empty() || self.out.contains(path) || self.vanilla.contains(path) {
            return Ok(());
        }
        self.copy_over(path)
    }

    /// Copy a WotLK file into the output as is, over 1.12's own if it has one.
    fn copy_over(&mut self, path: &str) -> Result<()> {
        if path.is_empty() || self.out.contains(path) {
            return Ok(());
        }
        match self.wotlk.try_read(path)? {
            Some(b) => self.out.add(path, b),
            None if !self.missing.iter().any(|m| m.eq_ignore_ascii_case(path)) => self.missing.push(path.to_owned()),
            None => {}
        }
        Ok(())
    }

    /// Copy a WotLK texture into the output at half its size (see [`blp`]).
    fn copy_halved(&mut self, path: &str) -> Result<()> {
        if path.is_empty() || self.out.contains(path) {
            return Ok(());
        }
        match self.wotlk.try_read(path)? {
            Some(b) => {
                let half = blp::halved(&b).with_context(|| format!("halve {path}"))?;
                self.out.add(path, half);
            }
            None if !self.missing.iter().any(|m| m.eq_ignore_ascii_case(path)) => self.missing.push(path.to_owned()),
            None => {}
        }
        Ok(())
    }

    /// Port a WotLK SoundEntries row and its files; the 1.12 id it has there. A 1.12 row of the
    /// same id and name is the same sound and is left as it is.
    pub fn sound(&mut self, id: u32) -> Result<u32> {
        if id == 0 {
            return Ok(0);
        }
        if let Some(&v) = self.sounds.get(&id) {
            return Ok(v);
        }
        let Some(row) = self.w("SoundEntries")?.find(id).cloned() else {
            eprintln!("note: WotLK SoundEntries has no row {id}");
            return Ok(0);
        };
        let wt = self.w("SoundEntries")?;
        let name = wt.s(&row, 2).to_owned();
        let mut fields = wt.fields_of(&row, &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 23]);
        let dir = wt.s(&row, 23).to_owned();
        let files: Vec<String> = (3..=12).map(|i| wt.s(&row, i).to_owned()).filter(|f| !f.is_empty()).collect();
        let vt = self.v("SoundEntries")?;
        let same = vt.find(id).map(|r| vt.s(r, 2) == name);
        let new_id = match same {
            Some(true) => {
                self.sounds.insert(id, id);
                return Ok(id);
            }
            Some(false) => self.v("SoundEntries")?.max_id() + 1,
            None => id,
        };
        self.sounds.insert(id, new_id);
        fields.truncate(29); // WotLK's SoundEntriesAdvancedID is last
        fields[0] = F::U(new_id);
        self.v("SoundEntries")?.push(&fields)?;
        for f in files {
            self.copy(&format!("{dir}\\{f}"))?;
        }
        Ok(new_id)
    }

    /// Port a CreatureSoundData row (38 to 30 columns: WotLK has a fifth fidget); its 1.12 id.
    fn creature_sound(&mut self, id: u32) -> Result<u32> {
        if id == 0 {
            return Ok(0);
        }
        let Some(row) = self.w("CreatureSoundData")?.find(id).cloned() else {
            return Ok(0);
        };
        let new_id = self.free_id("CreatureSoundData", id)?;
        let mut fields = Vec::with_capacity(30);
        fields.push(F::U(new_id));
        for v in 1..30usize {
            let w = if v <= 17 { v } else { v + 1 };
            let val = Dbc::u(&row, w);
            let out = match v {
                9 | 24 => val,     // footstep class, impact type
                22 => 0,           // NPCSounds id
                _ => self.sound(val)?,
            };
            fields.push(F::U(out));
        }
        self.v("CreatureSoundData")?.push(&fields)?;
        Ok(new_id)
    }

    /// Convert a WotLK model (`stem` without extension), its hard-coded textures and its event
    /// sounds into the output; false where WotLK has no such model.
    fn model(&mut self, stem: &str) -> Result<bool> {
        let path = format!("{stem}.m2");
        if self.out.contains(&path) {
            return Ok(true);
        }
        let Some(bytes) = self.wotlk.try_read(&path)? else {
            return Ok(false);
        };
        let skins = (0..m2::skin_count(&bytes)?)
            .map(|i| self.wotlk.read(&format!("{stem}{i:02}.skin")))
            .collect::<Result<Vec<_>>>()?;
        let mut anims = HashMap::new();
        for (id, var) in m2::anim_files(&bytes)? {
            if let Some(a) = self.wotlk.try_read(&format!("{stem}{id:04}-{var:02}.anim"))? {
                anims.insert((id, var), a);
            }
        }
        let (l, r, p) = m2::dropped(&bytes)?;
        self.dropped = (self.dropped.0 + l, self.dropped.1 + r, self.dropped.2 + p);
        let mut out = m2::convert(&m2::Wotlk { m2: &bytes, skins, anims }, &self.template)
            .with_context(|| format!("convert {stem}"))?;
        for name in m2::hardcoded_textures(&out)? {
            self.copy(&name)?;
        }
        // Event payloads that name sounds, re-pointed at their 1.12 ids.
        for (at, tag, id) in m2::events(&out)? {
            if SOUND_EVENTS.contains(&&tag) {
                let v = self.sound(id)?;
                out[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        self.out.add(&path, out);
        Ok(true)
    }

    /// What the races need beyond their rows: helms shaped for them, their death skeletons and
    /// WotLK's race icon sheet (under its own name; 1.12's keeps its 4 by 4 layout).
    pub fn race_extras(&mut self) -> Result<()> {
        let sheet = self.wotlk.read("Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Races.blp")?;
        self.out.add("Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Races-Wrath.blp", sheet);

        // Every 1.12 helm comes in a Human Male shape; WotLK has a Blood Elf and Draenei one.
        let mut helms: Vec<String> = self
            .vanilla
            .list()?
            .into_iter()
            .filter_map(|e| {
                let lower = e.name.to_ascii_lowercase();
                (lower.starts_with("item\\objectcomponents\\head\\") && lower.ends_with("_hum.m2"))
                    .then(|| e.name[..e.name.len() - "_HuM.m2".len()].to_owned())
            })
            .collect();
        helms.sort();
        helms.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        let (mut ported, mut lacking) = (0, 0);
        for base in &helms {
            for shape in ["BeM", "BeF", "DrM", "DrF"] {
                if self.model(&format!("{base}_{shape}"))? {
                    ported += 1;
                } else {
                    lacking += 1;
                }
            }
        }
        println!("helms: {ported} ported for {} 1.12 helms, {lacking} shapes WotLK lacks", helms.len());

        for race in ["BloodElf", "Draenei"] {
            for sex in ["Male", "Female"] {
                if !self.model(&format!("DeathSkeletons\\{race}{sex}DeathSkeleton"))? {
                    eprintln!("note: no {race} {sex} death skeleton");
                }
            }
        }
        Ok(())
    }

    /// Port a CreatureModelData row (28 to 16 columns) and its model; its 1.12 id.
    fn model_data(&mut self, id: u32) -> Result<u32> {
        let row = self.w("CreatureModelData")?.find(id).cloned().with_context(|| format!("no CreatureModelData {id}"))?;
        let name = self.w("CreatureModelData")?.s(&row, 2).to_owned();
        let stem = name.strip_suffix(".mdx").or_else(|| name.strip_suffix(".MDX")).unwrap_or(&name).to_owned();
        if !self.model(&stem)? {
            bail!("WotLK has no model {stem}");
        }
        let new_id = self.free_id("CreatureModelData", id)?;
        let sound = self.creature_sound(Dbc::u(&row, 13))?;
        let blood = Dbc::u(&row, 5);
        let blood = if self.v("UnitBlood")?.find(blood).is_some() { blood } else { 1 };
        let foot = Dbc::u(&row, 6);
        let foot = if self.v("FootprintTextures")?.find(foot).is_some() { foot } else { 1 };
        let mut f: Vec<F> = (0..16).map(|i| F::U(Dbc::u(&row, i))).collect();
        f[0] = F::U(new_id);
        f[1] = F::U(Dbc::u(&row, 1) & 0xff);
        f[2] = F::S(name);
        f[5] = F::U(blood);
        f[6] = F::U(foot);
        f[13] = F::U(sound);
        self.v("CreatureModelData")?.push(&f)?;
        Ok(new_id)
    }

    /// Port a CreatureDisplayInfo row (16 to 12 columns) and what it names; its 1.12 id.
    fn display(&mut self, id: u32) -> Result<u32> {
        let row = self.w("CreatureDisplayInfo")?.find(id).cloned().with_context(|| format!("no CreatureDisplayInfo {id}"))?;
        let texs: Vec<String> = (6..9).map(|i| self.wtables["CreatureDisplayInfo"].s(&row, i).to_owned()).collect();
        // A creature's skins sit beside its model, named without directory or extension.
        if let Some(cmd) = self.w("CreatureModelData")?.find(Dbc::u(&row, 1)).cloned() {
            let model_path = self.wtables["CreatureModelData"].s(&cmd, 2).to_owned();
            let dir = model_path.rsplit_once('\\').map_or("", |(d, _)| d).to_owned();
            for t in texs.iter().filter(|t| !t.is_empty()) {
                self.copy(&format!("{dir}\\{t}.blp"))?;
            }
        }
        let model = self.model_data(Dbc::u(&row, 1))?;
        let sound = self.creature_sound(Dbc::u(&row, 2))?;
        let new_id = self.free_id("CreatureDisplayInfo", id)?;
        let mut f = vec![F::U(new_id), F::U(model), F::U(sound), F::U(0), F::U(Dbc::u(&row, 4)), F::U(Dbc::u(&row, 5))];
        f.extend(texs.into_iter().map(F::S));
        f.extend([F::U(Dbc::u(&row, 10)), F::U(Dbc::u(&row, 11)), F::U(0)]);
        self.v("CreatureDisplayInfo")?.push(&f)?;
        Ok(new_id)
    }

    /// A `model` entry: one M2, over 1.12's own if it has one.
    pub fn model_entry(&mut self, stem: &str) -> Result<()> {
        if !self.model(stem)? {
            bail!("WotLK has no model {stem}.m2");
        }
        Ok(())
    }

    /// A `file` entry: any file, as is, over 1.12's own.
    pub fn file_entry(&mut self, path: &str) -> Result<()> {
        if self.wotlk.try_read(path)?.is_none() {
            bail!("WotLK has no file {path}");
        }
        self.copy_over(path)
    }

    /// A `creature` entry: a WotLK CreatureDisplayInfo row, its model, skins and sounds. A display
    /// id 1.12 already has is 1.12's own creature and is left alone.
    pub fn creature(&mut self, id: u32) -> Result<()> {
        if self.v("CreatureDisplayInfo")?.find(id).is_some() {
            self.ids.push(format!("creature {id} = display {id} (1.12's own, not ported)"));
            return Ok(());
        }
        let v = self.display(id)?;
        self.ids.push(format!("creature {id} = display {v}"));
        Ok(())
    }

    /// A `wmo` entry, and every WMO `fill` finds missing: the root and its group files as is
    /// (both versions are WMO v17), the textures 1.12 lacks, its doodads converted; false where
    /// WotLK has no such WMO.
    pub fn wmo(&mut self, path: &str) -> Result<bool> {
        if self.out.contains(path) {
            return Ok(true);
        }
        let Some(root) = self.wotlk.try_read(path)? else {
            return Ok(false);
        };
        let (mut groups, mut textures, mut doodads, mut sky) = (0usize, Vec::new(), Vec::new(), Vec::new());
        for (tag, data) in chunks(&root) {
            match tag.as_str() {
                "MOHD" if data.len() >= 8 => groups = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize,
                "MOTX" => textures = names(data),
                "MODN" => doodads = names(data),
                "MOSB" => sky = names(data),
                _ => {}
            }
        }
        let stem = &path[..path.len().saturating_sub(4)];
        for i in 0..groups {
            self.copy_over(&format!("{stem}_{i:03}.wmo"))?;
        }
        for t in &textures {
            self.copy(t)?;
        }
        for d in doodads.iter().chain(&sky) {
            let stem = crate::manifest::strip_m2(d);
            if !self.vanilla.contains(&format!("{stem}.m2")) && !self.model(stem)? {
                self.missing.push(format!("{stem}.m2"));
            }
        }
        self.out.add(path, root);
        Ok(true)
    }

    /// A `fill` entry: every model, WMO and ground texture the 1.12 tiles of `map` name but the
    /// 1.12 data lacks, from WotLK.
    pub fn fill(&mut self, map: &str) -> Result<()> {
        let prefix = format!("world\\maps\\{}\\", map.to_ascii_lowercase());
        let tiles: Vec<String> = self
            .vanilla
            .list()?
            .into_iter()
            .map(|e| e.name)
            .filter(|n| {
                let l = n.to_ascii_lowercase();
                l.starts_with(&prefix) && l.ends_with(".adt")
            })
            .collect();
        if tiles.is_empty() {
            bail!("the 1.12 data has no tiles of map {map}");
        }
        let (mut models, mut wmos, mut textures) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        for tile in &tiles {
            let b = self.vanilla.read(tile)?;
            for (tag, data) in chunks(&b) {
                let into = match tag.as_str() {
                    "MMDX" => &mut models,
                    "MWMO" => &mut wmos,
                    "MTEX" => &mut textures,
                    _ => continue,
                };
                into.extend(names(data).into_iter().map(|n| n.to_ascii_lowercase()));
            }
        }
        let (ported, lacking) = self.port_named(&models, &wmos, &textures)?;
        println!("fill {map}: {} tiles name {} models and {} WMOs; ported {ported} the 1.12 data lacks, {lacking} WotLK lacks too", tiles.len(), models.len(), wmos.len());
        Ok(())
    }

    /// Every model, WMO and texture named (lowercase) that the 1.12 data lacks, from WotLK:
    /// `(ported, lacking)`, the models and WMOs ported and those WotLK lacks too.
    fn port_named(&mut self, models: &BTreeSet<String>, wmos: &BTreeSet<String>, textures: &BTreeSet<String>) -> Result<(usize, usize)> {
        let (mut ported, mut lacking) = (0, 0);
        for m in models {
            let stem = crate::manifest::strip_m2(m);
            if self.vanilla.contains(&format!("{stem}.m2")) {
                continue;
            }
            if self.model(stem)? { ported += 1 } else { lacking += 1 }
        }
        for w in wmos {
            if self.vanilla.contains(w) {
                continue;
            }
            if self.wmo(w)? { ported += 1 } else { lacking += 1 }
        }
        for t in textures {
            self.copy(t)?;
        }
        Ok((ported, lacking))
    }

    /// A `tiles` entry: the map's WotLK tiles in the ranges that the 1.12 data lacks.
    pub fn tiles(&mut self, map: &str, x: (u32, u32), y: (u32, u32)) -> Result<()> {
        let want: Vec<(u32, u32)> = (y.0..=y.1).flat_map(|ty| (x.0..=x.1).map(move |tx| (tx, ty))).collect();
        self.port_tiles(map, &want, &format!("tiles {map} {}-{} {}-{}", x.0, x.1, y.0, y.1))
    }

    /// A `zone` entry: the map's WotLK tiles the 1.12 data lacks on which a chunk belongs to the
    /// WotLK AreaTable zone named (any of its rows on the map) or one of its subzones. Only the
    /// tiles inside the zone's WorldMapArea bounds are looked in, where it has one.
    pub fn zone(&mut self, map: &str, name: &str) -> Result<()> {
        let t = self.w("Map")?;
        let map_id = t
            .rows
            .iter()
            .find(|r| t.s(r, 1).eq_ignore_ascii_case(map))
            .map(|r| Dbc::u(r, 0))
            .with_context(|| format!("WotLK has no map {map}"))?;
        let t = self.w("AreaTable")?;
        let roots: Vec<u32> = t
            .rows
            .iter()
            .filter(|r| Dbc::u(r, 1) == map_id && t.s(r, 11).eq_ignore_ascii_case(name))
            .map(|r| Dbc::u(r, 0))
            .collect();
        if roots.is_empty() {
            bail!("WotLK's AreaTable has no zone {name} on map {map}");
        }
        let mut areas: HashSet<u32> = roots.iter().copied().collect();
        loop {
            let more: Vec<u32> = t
                .rows
                .iter()
                .filter(|r| areas.contains(&Dbc::u(r, 2)) && !areas.contains(&Dbc::u(r, 0)))
                .map(|r| Dbc::u(r, 0))
                .collect();
            if more.is_empty() {
                break;
            }
            areas.extend(more);
        }
        // WorldMapArea: left and right are world y, top and bottom world x, in yards.
        const TILE: f32 = 1600.0 / 3.0;
        let tile = |v: f32| (32.0 - v / TILE).floor().clamp(0.0, 63.0) as u32;
        let t = self.w("WorldMapArea")?;
        let bounds: Vec<((u32, u32), (u32, u32))> = t
            .rows
            .iter()
            .filter(|r| Dbc::u(r, 1) == map_id && roots.contains(&Dbc::u(r, 2)))
            .map(|r| {
                let f = |i| f32::from_bits(Dbc::u(r, i));
                ((tile(f(4)), tile(f(5))), (tile(f(6)), tile(f(7))))
            })
            .collect();
        let wdt = self.wotlk_wdt(map)?;
        let inside = |&(x, y): &(u32, u32)| {
            bounds.is_empty() || bounds.iter().any(|&(bx, by)| (bx.0..=bx.1).contains(&x) && (by.0..=by.1).contains(&y))
        };
        let mut want = Vec::new();
        for (x, y) in adt::wdt_tiles(&wdt).into_iter().filter(inside) {
            let path = tile_path(map, x, y);
            let bytes = match self.out.get(&path) {
                Some(b) => b.to_vec(),
                None if self.vanilla.contains(&path) => self.vanilla.read(&path)?,
                None => self.wotlk.read(&path)?,
            };
            if adt::area_ids(&bytes).iter().any(|a| areas.contains(a)) {
                want.push((x, y));
            }
        }
        if want.is_empty() {
            bail!("no WotLK tile of map {map} has a chunk of {name}");
        }
        self.port_tiles(map, &want, &format!("zone {map} {name}"))
    }

    fn wotlk_wdt(&mut self, map: &str) -> Result<Vec<u8>> {
        self.wotlk.try_read(&wdt_path(map))?.with_context(|| format!("WotLK has no map {map}"))
    }

    /// Convert the WotLK tiles of `want` that the 1.12 data lacks, mark them in the map's WDT
    /// (WotLK's, cleared, where 1.12 has no such map), and port what they name.
    fn port_tiles(&mut self, map: &str, want: &[(u32, u32)], what: &str) -> Result<()> {
        let wdt = self.wotlk_wdt(map)?;
        let big = adt::big_alpha(&wdt);
        let exists: HashSet<(u32, u32)> = adt::wdt_tiles(&wdt).into_iter().collect();
        let kinds: HashMap<u16, Liquid> =
            self.w("LiquidType")?.rows.iter().map(|r| (Dbc::u(r, 0) as u16, Liquid::from_type(Dbc::u(r, 3)))).collect();
        let (mut models, mut wmos, mut textures) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        let (mut tiles, mut own, mut absent, mut liquids) = (Vec::new(), 0, 0, 0);
        for &(x, y) in want {
            let path = tile_path(map, x, y);
            if !exists.contains(&(x, y)) {
                absent += 1;
            } else if self.vanilla.contains(&path) {
                own += 1;
            } else if self.out.contains(&path) {
                tiles.push((x, y));
            } else {
                let bytes = self.wotlk.read(&path)?;
                let out = adt::convert(&bytes, (x, y), big, &|id| kinds.get(&id).copied().unwrap_or(Liquid::Water))
                    .with_context(|| format!("convert {path}"))?;
                liquids += adt::check(&out).with_context(|| format!("benilla-adt rejects converted {path}"))?;
                let (m, w, t) = adt::named(&out);
                models.extend(m.into_iter().map(|n| n.to_ascii_lowercase()));
                wmos.extend(w.into_iter().map(|n| n.to_ascii_lowercase()));
                textures.extend(t.into_iter().map(|n| n.to_ascii_lowercase()));
                self.out.add(&path, out);
                tiles.push((x, y));
            }
        }
        if tiles.is_empty() && own == 0 {
            bail!("WotLK has none of these tiles of map {map}");
        }
        if !tiles.is_empty() {
            let path = wdt_path(map);
            let current = match self.out.get(&path) {
                Some(w) => Some(w.to_vec()),
                None if self.vanilla.contains(&path) => Some(self.vanilla.read(&path)?),
                None => None,
            };
            match current {
                Some(mut w) => {
                    if adt::mark_tiles(&mut w, &tiles)? || self.out.contains(&path) {
                        self.out.add(&path, w);
                    }
                }
                None => {
                    self.out.add(&path, adt::new_wdt(&wdt, &tiles)?);
                    println!("note: 1.12 has no map {map}: its WDT is ported, but not its Map.dbc, AreaTable or Light rows");
                }
            }
        }
        let (ported, lacking) = self.port_named(&models, &wmos, &textures)?;
        println!(
            "{what}: {} tiles converted with {liquids} liquid surfaces ({own} are 1.12's own, {absent} WotLK lacks); they name {} models and {} WMOs: ported {ported} the 1.12 data lacks, {lacking} WotLK lacks too",
            tiles.len(),
            models.len(),
            wmos.len()
        );
        Ok(())
    }

    /// Port one race: its ChrRaces row, bodies, skins, hair, classes, outfits and voices.
    pub fn race(&mut self, race: u32, faction: u32) -> Result<()> {
        let wrace = self.w("ChrRaces")?.find(race).cloned().with_context(|| format!("no WotLK race {race}"))?;
        let ws = |p: &mut Self, i: usize| p.wtables["ChrRaces"].s(&wrace, i).to_owned();
        let (prefix, file, name) = (ws(self, 6), ws(self, 11), ws(self, 14));
        let (facial0, facial1, hair) = (ws(self, 65), ws(self, 66), ws(self, 67));
        let male = self.display(Dbc::u(&wrace, 4))?;
        let female = self.display(Dbc::u(&wrace, 5))?;
        let exploration = self.sound(Dbc::u(&wrace, 3))?;
        let splash = self.sound(Dbc::u(&wrace, 10))?;

        // ChrRaces: the 1.12 Human row with this race's own values over it.
        let human = self.v("ChrRaces")?.find(1).cloned().context("no 1.12 Human")?;
        let mut f = self.tables["ChrRaces"].fields_of(&human, &[6, 15, 17, 26, 27, 28]);
        if self.v("ChrRaces")?.find(race).is_some() {
            bail!("1.12 ChrRaces already has race {race}");
        }
        f[0] = F::U(race);
        f[1] = F::U(Dbc::u(&wrace, 1));
        f[2] = F::U(faction);
        f[3] = F::U(exploration);
        f[4] = F::U(male);
        f[5] = F::U(female);
        f[6] = F::S(prefix);
        f[8] = F::U(Dbc::u(&wrace, 7));
        f[9] = F::U(Dbc::u(&wrace, 8));
        f[12] = F::U(Dbc::u(&wrace, 9));
        f[13] = F::U(splash);
        f[15] = F::S(file);
        f[16] = F::U(0);
        f[17] = F::S(name);
        f[26] = F::S(facial0);
        f[27] = F::S(facial1);
        f[28] = F::S(hair);
        self.v("ChrRaces")?.push(&f)?;

        self.sections(race)?;
        self.hair_geosets(race)?;
        self.facial_hair(race)?;
        let classes = self.classes(race)?;
        self.outfits(race, &classes)?;
        self.voices(race)?;
        Ok(())
    }

    /// CharSections: WotLK's order (textures, flags, variation, colour) to 1.12's (variation,
    /// colour, textures, flags), with the textures. Death Knight rows are left out; a WotLK row
    /// not marked playable is marked not selectable in 1.12. Every texture the compositor paints
    /// into the body is halved to 1.12's size; the hair mesh's own texture (hair, first) is not.
    fn sections(&mut self, race: u32) -> Result<()> {
        for row in self.w_rows("CharSections", 1, race)? {
            let wflags = Dbc::u(&row, 7);
            if wflags & 0x4 != 0 {
                continue;
            }
            let texs: Vec<String> = (4..7).map(|i| self.wtables["CharSections"].s(&row, i).to_owned()).collect();
            let section = Dbc::u(&row, 3);
            for (i, t) in texs.iter().enumerate() {
                if section == 3 && i == 0 {
                    self.copy(t)?;
                } else {
                    self.copy_halved(t)?;
                }
            }
            let id = self.v("CharSections")?.max_id() + 1;
            let mut f = vec![F::U(id), F::U(race), F::U(Dbc::u(&row, 2)), F::U(Dbc::u(&row, 3)), F::U(Dbc::u(&row, 8)), F::U(Dbc::u(&row, 9))];
            f.extend(texs.into_iter().map(F::S));
            f.push(F::U(if wflags & 0x1 != 0 { 0 } else { 1 }));
            self.v("CharSections")?.push(&f)?;
        }
        Ok(())
    }

    /// CharHairGeosets: the same six columns.
    fn hair_geosets(&mut self, race: u32) -> Result<()> {
        for row in self.w_rows("CharHairGeosets", 1, race)? {
            let id = self.v("CharHairGeosets")?.max_id() + 1;
            let mut f: Vec<F> = (0..6).map(|i| F::U(Dbc::u(&row, i))).collect();
            f[0] = F::U(id);
            self.v("CharHairGeosets")?.push(&f)?;
        }
        Ok(())
    }

    /// CharacterFacialHairStyles: WotLK's geosets at 3..5 to 1.12's at 6..8 (1xx, 3xx, 2xx).
    fn facial_hair(&mut self, race: u32) -> Result<()> {
        for row in self.w_rows("CharacterFacialHairStyles", 0, race)? {
            let u = |i| F::U(Dbc::u(&row, i));
            let f = vec![u(0), u(1), u(2), F::U(0), F::U(0), F::U(0), u(3), u(4), u(5)];
            self.v("CharacterFacialHairStyles")?.push(&f)?;
        }
        Ok(())
    }

    /// CharBaseInfo: the race's classes, Death Knight left out. Two-byte rows.
    fn classes(&mut self, race: u32) -> Result<Vec<u8>> {
        let classes: Vec<u8> = self
            .w("CharBaseInfo")?
            .rows
            .iter()
            .filter(|r| u32::from(r[0]) == race && r[1] != DEATH_KNIGHT)
            .map(|r| r[1])
            .collect();
        let t = self.v("CharBaseInfo")?;
        for &c in &classes {
            t.rows.push(vec![race as u8, c]);
        }
        Ok(classes)
    }

    /// CharStartOutfit: WotLK's outfits are Burning Crusade items 1.12 has no displays for, so
    /// each class and sex wears the 1.12 outfit another race of that class starts in.
    fn outfits(&mut self, race: u32, classes: &[u8]) -> Result<()> {
        for &class in classes {
            for sex in 0..2u8 {
                let t = self.v("CharStartOutfit")?;
                let Some(mut row) = t.rows.iter().find(|r| r[5] == class && r[6] == sex).cloned() else {
                    eprintln!("note: no 1.12 outfit for class {class} sex {sex}");
                    continue;
                };
                let id = t.max_id() + 1;
                row[0..4].copy_from_slice(&id.to_le_bytes());
                row[4] = race as u8;
                t.rows.push(row);
            }
        }
        Ok(())
    }

    /// VocalUISounds and EmotesTextSound rows, with their sounds. A voice line past 1.12's
    /// last (`0x43`) is one 1.12 never speaks, and is left out.
    fn voices(&mut self, race: u32) -> Result<()> {
        for row in self.w_rows("VocalUISounds", 2, race)? {
            if Dbc::u(&row, 1) >= 0x44 {
                continue;
            }
            let id = self.v("VocalUISounds")?.max_id() + 1;
            let mut f = vec![F::U(id), F::U(Dbc::u(&row, 1)), F::U(race)];
            for i in 3..7 {
                f.push(F::U(self.sound(Dbc::u(&row, i))?));
            }
            self.v("VocalUISounds")?.push(&f)?;
        }
        for row in self.w_rows("EmotesTextSound", 2, race)? {
            let id = self.v("EmotesTextSound")?.max_id() + 1;
            let sound = self.sound(Dbc::u(&row, 4))?;
            let f = vec![F::U(id), F::U(Dbc::u(&row, 1)), F::U(race), F::U(Dbc::u(&row, 3)), F::U(sound)];
            self.v("EmotesTextSound")?.push(&f)?;
        }
        Ok(())
    }

    /// Put every extended 1.12 table into the output.
    pub fn finish(mut self) -> (MpqOut, Vec<String>) {
        // UnitBlood and FootprintTextures are only looked in.
        for (name, t) in self.tables.iter().filter(|(n, _)| !matches!(n.as_str(), "UnitBlood" | "FootprintTextures")) {
            self.out.add(&dbc_path(name), t.to_bytes());
        }
        (self.out, self.missing)
    }
}