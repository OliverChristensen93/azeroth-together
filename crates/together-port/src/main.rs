//! `together-port`: brings WotLK 3.3.5 art into Azeroth Together's 1.12 data, rewritten in 1.12's
//! formats, as `WoW\Data\patch-W.MPQ`. What it ports is the machine's shared wish list,
//! `WoW\wotlk-port.txt` ([`manifest`]). See `docs/WOTLK-PORT.md`.
//!
//! ```text
//! together-port add <kind> <arg>   add to the wish list and rebuild (kinds: manifest.rs)
//! together-port build              rebuild the archive from the wish list
//! together-port list               show the wish list
//! ```

mod adt;
mod blp;
mod dbc;
mod m2;
mod manifest;
mod mpq_out;
mod port;
mod source;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use manifest::{Entry, Manifest, Paths};

fn usage() -> ! {
    eprintln!(
        "usage:\n  together-port add <race|fill|wmo|model|creature|file|tiles|zone|wotlk> <arg>\n  together-port build\n  together-port list\n\
         dev:\n  together-port model <wotlk Data> <vanilla Data> <model without .m2> <out.m2>\n  together-port dbc <wotlk Data> <vanilla Data> <wotlk|vanilla> <Name> <column> <value>\n  \
         together-port extract <wotlk Data> <path> <out>\n  together-port sizes <wotlk Data> <race>\n  together-port vlist <vanilla Data> <substring>\n  \
         together-port vextract <vanilla Data> <path> <out>\n  together-port effects <file.m2>\n  \
         together-port spot <Map> <x> <y>   what the game data has at WoW (x, y): for a zone's centre"
    );
    std::process::exit(2);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("add") if args.len() >= 3 => {
            let paths = Paths::find()?;
            let mut m = Manifest::load(&paths.manifest())?;
            let (kind, arg) = (args[1].as_str(), args[2..].join(" "));
            let line = if kind == "wotlk" {
                if !Path::new(&arg).join("common.MPQ").exists() {
                    bail!("{arg} has no common.MPQ: give the WotLK client's Data folder");
                }
                format!("wotlk {arg}")
            } else {
                Entry::parse(kind, &arg)?.line()
            };
            if m.add(line.clone()) {
                m.save()?;
                println!("added `{line}` to {}", paths.manifest().display());
            } else {
                println!("`{line}` is already on the list");
            }
            if kind == "wotlk" {
                return Ok(());
            }
            build(&paths)
        }
        Some("build") if args.len() == 1 => build(&Paths::find()?),
        Some("list") if args.len() == 1 => {
            let paths = Paths::find()?;
            let m = Manifest::load(&paths.manifest())?;
            println!("{}", paths.manifest().display());
            match m.wotlk() {
                Some(w) => println!("wotlk {}", w.display()),
                None => println!("wotlk (not set: together-port add wotlk <Data folder>)"),
            }
            for e in m.entries()? {
                println!("{}", e.line());
            }
            Ok(())
        }
        Some("model") if args.len() == 5 => {
            let mut wotlk = source::Wotlk::open(&PathBuf::from(&args[1]))?;
            let vanilla = benilla_formats::Chain::open(&PathBuf::from(&args[2]))?;
            let template = m2::read_playable(&vanilla.read(port::PLAYABLE_TEMPLATE)?)?;
            let out = port_model(&mut wotlk, &args[3], &template)?;
            check_model(&out)?;
            std::fs::write(&args[4], &out)?;
            Ok(())
        }
        Some("dbc") if args.len() == 7 => {
            let path = format!("DBFilesClient\\{}.dbc", args[4]);
            let bytes = if args[3] == "wotlk" {
                source::Wotlk::open(&PathBuf::from(&args[1]))?.read(&path)?
            } else {
                benilla_formats::Chain::open(&PathBuf::from(&args[2]))?.read(&path)?
            };
            let t = dbc::Dbc::parse(&bytes)?;
            println!("{} rows, {} fields, {} bytes per row", t.rows.len(), t.fields, t.record_size);
            let (col, val): (usize, u32) = (args[5].parse()?, args[6].parse()?);
            for r in t.rows.iter().filter(|r| dbc::Dbc::u(r, col) == val) {
                println!("{}", t.dump(r));
            }
            Ok(())
        }
        Some("extract") if args.len() == 4 => {
            let bytes = source::Wotlk::open(&PathBuf::from(&args[1]))?.read(&args[2])?;
            std::fs::write(&args[3], bytes)?;
            Ok(())
        }
        Some("sizes") if args.len() == 3 => {
            let mut w = source::Wotlk::open(&PathBuf::from(&args[1]))?;
            let t = dbc::Dbc::parse(&w.read("DBFilesClient\\CharSections.dbc")?)?;
            let race: u32 = args[2].parse()?;
            let mut seen = std::collections::HashSet::new();
            for r in t.rows.iter().filter(|r| dbc::Dbc::u(r, 1) == race) {
                for i in 4..7 {
                    let name = t.s(r, i).to_owned();
                    let key = (dbc::Dbc::u(r, 2), dbc::Dbc::u(r, 3), i);
                    if name.is_empty() || !seen.insert(key) {
                        continue;
                    }
                    let size = w.try_read(&name)?.map(|b| {
                        (u32::from_le_bytes(b[12..16].try_into().unwrap()), u32::from_le_bytes(b[16..20].try_into().unwrap()))
                    });
                    println!("sex {} section {} tex{}: {size:?} {name}", key.0, key.1, i - 4);
                }
            }
            Ok(())
        }
        Some("vextract") if args.len() == 4 => {
            let bytes = benilla_formats::Chain::open(&PathBuf::from(&args[1]))?.read(&args[2])?;
            std::fs::write(&args[3], bytes)?;
            Ok(())
        }
        Some("effects") if args.len() == 2 => {
            // A v256 model's emitters, ribbons and lights as the client reads them.
            let bytes = std::fs::read(&args[1])?;
            for (i, e) in benilla_formats::parse_m2_particle_emitters(&bytes)?.iter().enumerate() {
                let l = &e.over_life;
                println!(
                    "emitter {i}: flags {:#x} bone {} at {:?} {:?} {:?} lit {} tex {:?} tiles {}x{} model {:?}",
                    e.flags, e.bone, e.position, e.shape, e.blend, e.lit, e.texture, e.tile_rows, e.tile_cols, e.geometry_model
                );
                println!(
                    "  rate peak {} lifespan peak {} mid {} colour {:?} size {:?} head {:?} tail {:?} repeat {:?}",
                    e.timing.peak_rate(), e.params.peak_lifespan(), l.mid, l.color, l.scale, l.head_cells, l.tail_cells, l.repeat
                );
                println!(
                    "  tail {} twinkle {} {} {}..{} drag {} spin {} tumble {:?}..{:?} follow {} {} {} {}",
                    e.tail_time, e.twinkle_speed, e.twinkle_percent, e.twinkle_min, e.twinkle_max, e.drag, e.spin,
                    e.angular_velocity_min, e.angular_velocity_max, e.follow_speed1, e.follow_scale1, e.follow_speed2, e.follow_scale2
                );
            }
            for (i, r) in benilla_formats::parse_m2_ribbon_emitters(&bytes)?.iter().enumerate() {
                println!("ribbon {i}: bone {} edges/s {} life {} gravity {}", r.bone, r.edges_per_second, r.edge_lifetime, r.gravity);
            }
            for (i, l) in benilla_formats::parse_m2_lights(&bytes).iter().enumerate() {
                println!("light {i}: {l:?}");
            }
            Ok(())
        }
        Some("spot") if args.len() == 4 => {
            let mut chain = benilla_formats::Chain::open(&Paths::find()?.vanilla())?;
            spot(&mut chain, &args[1], args[2].parse()?, args[3].parse()?)
        }
        // `loadcheck <archive> [filter]`: every model in an archive through the parse calls the
        // engine's M2 loader makes, each caught if it panics and timed, to find one that would
        // leave its load (and the loading screen) hanging without an error.
        Some("loadcheck") if args.len() >= 2 => {
            let a = benilla_mpq::Archive::open(&args[1])?;
            let list = String::from_utf8(a.read_file("(listfile)")?)?;
            let want = args.get(2).map(|f| f.to_ascii_lowercase());
            let (mut n, mut bad) = (0, 0);
            for name in list.lines().filter(|l| l.to_ascii_lowercase().ends_with(".m2")) {
                if want.as_ref().is_some_and(|w| !name.to_ascii_lowercase().contains(w.as_str())) {
                    continue;
                }
                let bytes = a.read_file(name)?;
                let start = std::time::Instant::now();
                let r = std::panic::catch_unwind(|| {
                    let subs = benilla_formats::parse_m2_render_submeshes(&bytes, "", &[]).map(|s| s.len());
                    let _ = benilla_formats::parse_m2_bounds(&bytes);
                    let _ = benilla_formats::parse_m2_collision_hull(&bytes);
                    let _ = benilla_formats::parse_m2_lights(&bytes);
                    let _ = benilla_formats::parse_m2_skeleton(&bytes);
                    let e = benilla_formats::parse_m2_particle_emitters(&bytes).map(|e| e.len());
                    let r = benilla_formats::parse_m2_ribbon_emitters(&bytes).map(|r| r.len());
                    let _ = benilla_formats::parse_m2_attachments(&bytes);
                    let _ = benilla_formats::parse_m2_event_markers(&bytes);
                    let _ = benilla_formats::parse_m2_animations(&bytes);
                    (subs.map_err(|e| e.to_string()), e.map_err(|e| e.to_string()), r.map_err(|e| e.to_string()))
                });
                let ms = start.elapsed().as_millis();
                n += 1;
                match r {
                    Err(_) => {
                        bad += 1;
                        println!("PANIC {name}");
                    }
                    Ok((s, e, r)) if s.is_err() || e.is_err() || r.is_err() || ms > 500 => {
                        bad += 1;
                        println!("{name}: submeshes {s:?}, emitters {e:?}, ribbons {r:?}, {ms} ms");
                    }
                    Ok(_) => {}
                }
            }
            println!("{n} models, {bad} flagged");
            Ok(())
        }
        Some("vlist") if args.len() == 3 => {
            let chain = benilla_formats::Chain::open(&PathBuf::from(&args[1]))?;
            let want = args[2].to_ascii_lowercase();
            for e in chain.list()?.iter().filter(|e| e.name.to_ascii_lowercase().contains(&want)) {
                println!("{} {}", e.size, e.name);
            }
            Ok(())
        }
        _ => usage(),
    }
}

/// What the game's data (1.12 and the installed port) has at WoW `(x, y)` on `map`: the tile,
/// whether it is in the data, the area, the ground height and any water there. For picking a
/// zone's centre (docs/ADDING-ZONES.md).
fn spot(chain: &mut benilla_formats::Chain, map: &str, x: f32, y: f32) -> Result<()> {
    let (tx, ty) = benilla_wdt::world_to_tile(x, y);
    println!("tile {map}_{tx}_{ty}");
    let Ok(mesh) = benilla_formats::load_tile_mesh(chain, map, tx, ty) else {
        println!("  not in the game data: port its terrain with `together-port add zone` or `tiles`");
        return Ok(());
    };
    let wow = [x, y, 0.0];
    let areas = dbc::Dbc::parse(&chain.read("DBFilesClient\\AreaTable.dbc")?)?;
    if let Some(id) = benilla_formats::area_id_at(&mesh.chunks, wow) {
        let name = areas.find(id).map_or("", |r| areas.s(r, 11));
        println!("  area {id} {name}");
    }
    match benilla_formats::terrain_height_at(&mesh.chunks, wow) {
        Some(z) => println!("  ground z {z:.1}: a zone centred here wants z about {:.0}", z + 2.0),
        None => println!("  no ground here (a hole in the terrain: a cave mouth or a building's floor)"),
    }
    // The chunk's north-west corner is its first vertex; a chunk is 100/3 yd a side.
    let chunk = mesh.chunks.iter().find(|c| {
        c.positions.first().is_some_and(|nw| x <= nw[0] && x > nw[0] - 100.0 / 3.0 && y <= nw[1] && y > nw[1] - 100.0 / 3.0)
    });
    for l in chunk.map_or(&[][..], |c| &c.liquids[..]) {
        let top = l.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        println!("  {:?} in this chunk, surface at z {top:.1}", l.kind);
    }
    println!("  {} doodads and {} buildings on the tile", mesh.doodads.len(), mesh.wmos.len());
    Ok(())
}

/// Holds `WoW\wotlk-port.lock` while a build runs, so two sessions never write the archive at
/// once; a stale lock (a killed build) is removed by hand.
struct Lock(PathBuf);

impl Lock {
    fn take(path: PathBuf) -> Result<Self> {
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).with_context(|| {
            format!("{} exists: another session is building the port. If none is, delete it.", path.display())
        })?;
        Ok(Self(path))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Build the archive from the wish list. The archive in place is moved aside while the 1.12
/// chain is read, so a build always starts from vanilla, and put back if the build fails.
fn build(paths: &Paths) -> Result<()> {
    let _lock = Lock::take(paths.lock())?;
    let m = Manifest::load(&paths.manifest())?;
    let wotlk_dir = m.wotlk().context(
        "no WotLK client set: `together-port add wotlk <its Data folder>` (or set WOTLK_DATA)",
    )?;
    let entries = m.entries()?;
    let out = paths.archive();
    let aside = out.with_extension("previous");
    if out.exists() {
        std::fs::rename(&out, &aside)?;
    }
    let result = (|| -> Result<()> {
        let mut wotlk = source::Wotlk::open(&wotlk_dir)?;
        let vanilla = benilla_formats::Chain::open(&paths.vanilla())?;
        let mut p = port::Port::new(&mut wotlk, &vanilla)?;
        let mut races = false;
        for e in &entries {
            let done = match e {
                Entry::Race(r) => {
                    races = true;
                    let faction = port::RACES.iter().find(|(id, _)| id == r).map(|&(_, f)| f).unwrap_or(1);
                    p.race(*r, faction)
                }
                Entry::Fill(map) => p.fill(map),
                Entry::Wmo(path) => p.wmo(path).and_then(|ok| if ok { Ok(()) } else { bail!("WotLK has no {path}") }),
                Entry::Model(stem) => p.model_entry(stem),
                Entry::Creature(id) => p.creature(*id),
                Entry::File(path) => p.file_entry(path),
                Entry::Tiles { map, x, y } => p.tiles(map, *x, *y),
                Entry::Zone { map, name } => p.zone(map, name),
                Entry::Minimap(_) => Ok(()), // after every other entry, below
            };
            done.with_context(|| format!("`{}`", e.line()))?;
        }
        if races {
            p.race_extras()?;
        }
        // Minimaps last, so they see every tile and WMO the other entries ported.
        for e in &entries {
            if let Entry::Minimap(map) = e {
                p.minimap(map).with_context(|| format!("`{}`", e.line()))?;
            }
        }
        let (tinted, resampled) = p.approximated;
        let ids = std::mem::take(&mut p.ids);
        let (archive, missing) = p.finish();
        if !missing.is_empty() {
            println!("{} named files are in neither the 1.12 nor the WotLK data (WotLK skips them too)", missing.len());
            if std::env::var_os("TOGETHER_PORT_MISSING").is_some() {
                missing.iter().for_each(|m| println!("  missing: {m}"));
            }
        }
        if tinted + resampled > 0 {
            println!(
                "particle emitters carried in part: {tinted} lose their ParticleColor tint,                  {resampled} have an over-life curve resampled to three keys"
            );
        }
        let files = archive.len();
        archive.write(&out)?;
        verify(&out)?;
        let mut ids_text = String::from("# Ids `together-port build` gave the wish list's entries.\n");
        for line in &ids {
            println!("{line}");
            ids_text.push_str(line);
            ids_text.push('\n');
        }
        std::fs::write(paths.ids(), ids_text)?;
        println!("wrote {} ({files} files from {} wish-list entries)", out.display(), entries.len());
        Ok(())
    })();
    match result {
        Ok(()) => {
            let _ = std::fs::remove_file(&aside);
            Ok(())
        }
        Err(e) => {
            if aside.exists() {
                let _ = std::fs::rename(&aside, &out);
            }
            Err(e)
        }
    }
}

/// Read every file of a written archive back with the client's own MPQ reader, every model with
/// its M2 reader, every WMO with its WMO reader and every terrain tile with its ADT reader.
fn verify(path: &Path) -> Result<()> {
    let a = benilla_mpq::Archive::open(path).context("benilla-mpq rejects the archive")?;
    let list = String::from_utf8(a.read_file("(listfile)")?)?;
    let mut effects = [0usize; 3];
    for name in list.lines().filter(|l| !l.is_empty()) {
        let bytes = a.read_file(name).with_context(|| format!("benilla-mpq cannot read {name}"))?;
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".m2") {
            check_model(&bytes).with_context(|| name.to_owned())?;
            let counts = [0x13c, 0x134, 0x11c].map(|at| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize);
            if counts != [0; 3] && std::env::var_os("TOGETHER_PORT_EFFECTS").is_some() {
                println!("  effects: {name}: {counts:?}");
            }
            for (n, c) in effects.iter_mut().zip(counts) {
                *n += c;
            }
        } else if lower.ends_with(".adt") {
            adt::check(&bytes).with_context(|| format!("benilla-adt rejects {name}"))?;
        } else if lower.ends_with(".wdt") {
            anyhow::ensure!(!adt::wdt_tiles(&bytes).is_empty(), "{name} marks no tiles");
        } else if lower.ends_with(".wmo") {
            benilla_wmo::parse_wmo(&mut Cursor::new(bytes.as_slice()))
                .map_err(|e| anyhow::anyhow!("{e:?}"))
                .with_context(|| format!("benilla-wmo rejects {name}"))?;
        }
    }
    let [particles, ribbons, lights] = effects;
    println!("the converted models carry {particles} particle emitters, {ribbons} ribbons and {lights} lights");
    Ok(())
}

/// Read a WotLK model with its skins and `.anim` files and rewrite it for 1.12.
fn port_model(wotlk: &mut source::Wotlk, stem: &str, template: &[m2::Playable]) -> Result<Vec<u8>> {
    let bytes = wotlk.read(&format!("{stem}.m2"))?;
    let skins = (0..m2::skin_count(&bytes)?)
        .map(|i| wotlk.read(&format!("{stem}{i:02}.skin")))
        .collect::<Result<Vec<_>>>()?;
    let mut anims = HashMap::new();
    for (id, var) in m2::anim_files(&bytes)? {
        match wotlk.try_read(&format!("{stem}{id:04}-{var:02}.anim"))? {
            Some(a) => {
                anims.insert((id, var), a);
            }
            None => eprintln!("note: {stem}: no .anim for {id}-{var}; that sequence keeps the rest pose"),
        }
    }
    let (tinted, resampled) = m2::approximated(&bytes)?;
    if tinted + resampled > 0 {
        eprintln!("note: {stem}: {tinted} emitters lose their tint, {resampled} have curves resampled to three keys");
    }
    m2::convert(&m2::Wotlk { m2: &bytes, skins, anims }, template).with_context(|| format!("convert {stem}"))
}

/// Read a converted model back with the client's own readers. Quiet: an error says what failed.
fn check_model(bytes: &[u8]) -> Result<()> {
    let format = benilla_m2::parse_m2(&mut Cursor::new(bytes)).context("benilla-m2 rejects the model")?;
    format.model().parse_embedded_skin(bytes, 0).context("skin 0")?;
    let anims = benilla_formats::parse_m2_animations(bytes);
    // Every rotation key a unit quaternion, or the decoding is wrong.
    let worst = anims
        .iter()
        .flat_map(|a| &a.bones)
        .flat_map(|b| &b.rotation)
        .map(|(_, q)| (q.iter().map(|c| c * c).sum::<f32>().sqrt() - 1.0).abs())
        .fold(0.0f32, f32::max);
    anyhow::ensure!(worst < 0.01, "a rotation key {worst} away from unit length");
    m2::check_effects(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Stormwind's brazier, which 1.12 and WotLK both ship: its WotLK emitters, converted and
    /// read back with the client's reader, burn like 1.12's own. Skipped without both clients.
    #[test]
    fn a_ported_brazier_burns_like_the_1_12_one() {
        let Some((paths, wotlk_dir)) = Paths::find()
            .ok()
            .and_then(|p| Manifest::load(&p.manifest()).ok()?.wotlk().map(|w| (p, w)))
            .filter(|(p, w)| p.vanilla().exists() && w.exists())
        else {
            eprintln!("skipped: no 1.12 and WotLK data");
            return;
        };
        let stem = r"World\Generic\Human\Passive Doodads\Braziers\StormwindBrazier01";
        let mut wotlk = source::Wotlk::open(&wotlk_dir).unwrap();
        let vanilla = benilla_formats::Chain::open(&paths.vanilla()).unwrap();
        let template = m2::read_playable(&vanilla.read(port::PLAYABLE_TEMPLATE).unwrap()).unwrap();
        let ported = port_model(&mut wotlk, stem, &template).unwrap();
        check_model(&ported).unwrap();
        let ours = benilla_formats::parse_m2_particle_emitters(&ported).unwrap();
        let theirs = benilla_formats::parse_m2_particle_emitters(&vanilla.read(&format!("{stem}.m2")).unwrap()).unwrap();
        assert_eq!(ours.len(), 2);
        assert_eq!(ours.len(), theirs.len());
        // Bones differ (WotLK's skeleton), and WotLK zeroed the follow speeds; the rest is 1.12's.
        let look = |e: &benilla_formats::ParticleEmitterDef| {
            format!(
                "{:#x} {:?} {:?} {:?} {} {:?} {}x{} {} {} {:?} tail {} twinkle {} {} {} {} drag {} spin {} rate {} life {}",
                e.flags, e.position, e.shape, e.blend, e.lit, e.texture, e.tile_rows, e.tile_cols, e.head_tail,
                e.over_life.mid, (e.over_life.color, e.over_life.scale, e.over_life.head_cells, e.over_life.tail_cells, e.over_life.repeat),
                e.tail_time, e.twinkle_speed, e.twinkle_percent, e.twinkle_min, e.twinkle_max, e.drag, e.spin,
                e.timing.peak_rate(), e.params.peak_lifespan()
            )
        };
        for (a, b) in ours.iter().zip(&theirs) {
            assert_eq!(look(a), look(b));
        }
    }
}
