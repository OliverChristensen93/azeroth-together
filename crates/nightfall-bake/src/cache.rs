//! The bake's cache: `<dir>/<key>-<hash>.bin`, deflated. The hash covers what the bake reads
//! (every MPQ's name, size and time, so a patched or swapped install bakes again), what it is
//! asked (the map, the bounds, the props) and [`BAKE_VERSION`]. A file that does not read back
//! whole (damaged, cut short, or from another codec version) is baked again and overwritten; older
//! files for the same map are removed once a new one is written. A bake that could not read some
//! file the data has is not cached, so the next launch tries again.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use nightfall_sim::{HeightMap, CELL};

use crate::bake::{bake, BakeOutput, BakeRequest};

/// Bumped whenever the bake would make a different map from the same data, or a cache baked
/// before would be read back as a map it no longer is. What forces a bump:
///
/// - `nightfall-sim`'s [`MERGE_GAP`](nightfall_sim::MERGE_GAP) and
///   [`TERRAIN_DEPTH`](nightfall_sim::TERRAIN_DEPTH), which shape every column's spans;
/// - any change to the rasterizing (`raster.rs`: which columns a triangle covers, how a hull or
///   a building fills them), to placement (`place.rs`), or to what the bake takes in (which
///   models, sets or faces, the placement radii, the bounds' blocking);
/// - [`CELL`], the column size (it is also in the key itself, so a cache never mixes sizes, but
///   bump it anyway so the change is seen);
/// - `benilla-formats`' collision output: the terrain heights, the WMO walking-collision and the
///   M2 hull triangles it hands the bake, after an upstream merge that touches them.
///
/// [`HEADROOM`](nightfall_sim::HEADROOM) and [`MAX_STEP`](nightfall_sim::MAX_STEP) do not:
/// they are read from the spans at run time. A codec change does not either: a file the codec
/// rejects is baked again.
pub const BAKE_VERSION: u32 = 2;

/// A height map, and whether it came from the cache.
pub struct Baked {
    pub map: HeightMap,
    pub from_cache: bool,
    /// Files the data has that the bake could not read (see [`BakeOutput::failures`]); such a map
    /// plays but is not cached.
    pub failures: Vec<String>,
    /// Why a good map could not be written to the cache, if it could not.
    pub cache_error: Option<String>,
}

/// FNV-1a over the parts, each ended by a zero byte, as `together_net::content_hash` hashes.
fn fnv(parts: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// Every MPQ in the data folder as `name:size:seconds`, sorted: what changes when the install
/// does, without reading gigabytes.
fn data_fingerprint(dir: &Path) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with(".mpq") {
            continue;
        }
        // Through symlinks, as `Chain::open` reads them: a link pointed at another archive must
        // change the key. A link to nothing is not an archive the chain would open either.
        let Ok(meta) = std::fs::metadata(entry.path()) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        parts.push(format!("{name}:{}:{secs}", meta.len()));
    }
    parts.sort();
    Ok(parts)
}

/// The cache's key for a request.
pub fn cache_key(req: &BakeRequest) -> Result<u64> {
    let mut parts = vec![
        format!("version {BAKE_VERSION}"),
        format!("cell {CELL}"),
        format!("map {} {}", req.key, req.map_id),
    ];
    parts.extend(
        req.bounds
            .points()
            .iter()
            .map(|p| format!("bound {} {}", p.x.to_bits(), p.y.to_bits())),
    );
    parts.extend(req.props.iter().map(|p| {
        format!(
            "prop {} {:?} {:?} {}",
            p.model.to_ascii_lowercase(),
            p.position,
            p.rotation,
            p.scale
        )
    }));
    parts.extend(data_fingerprint(req.data_dir)?);
    Ok(fnv(&parts))
}

/// Where a request's map is cached in `dir`.
pub fn cache_file(dir: &Path, req: &BakeRequest) -> Result<PathBuf> {
    Ok(dir.join(format!("{}-{:016x}.bin", req.key, cache_key(req)?)))
}

/// The map in a cache file, if it reads back whole.
pub(crate) fn read_cache(path: &Path) -> Option<HeightMap> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    DeflateDecoder::new(BufReader::new(file))
        .read_to_end(&mut bytes)
        .ok()?;
    HeightMap::from_bytes(&bytes).ok()
}

/// Whether `name` is one of `key`'s cache files: `<key>-<16 hex digits>.bin`, or the temporary
/// `<key>-<16 hex digits>.<process id>.tmp` of a write that never finished. Matched exactly, so
/// a map whose key begins with this one's (`elwynn-forest` for `elwynn`) is left alone.
fn is_cache_name(key: &str, name: &str) -> bool {
    let Some(rest) = name.strip_prefix(key).and_then(|r| r.strip_prefix('-')) else {
        return false;
    };
    let Some((hash, tail)) = rest.split_at_checked(16) else {
        return false;
    };
    hash.bytes().all(|b| b.is_ascii_hexdigit())
        && (tail == ".bin"
            || tail
                .strip_prefix('.')
                .and_then(|t| t.strip_suffix(".tmp"))
                .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit())))
}

/// Write the map to `path` (through a temporary file of this process's own, so a crash never
/// leaves half a map under the real name and two launches never share a temporary), then remove
/// this map's older cache files and stale temporaries in `dir`.
pub(crate) fn write_cache(
    dir: &Path,
    key: &str,
    path: &Path,
    map: &HeightMap,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let written = (|| {
        {
            let file = std::fs::File::create(&tmp)?;
            let mut out = DeflateEncoder::new(BufWriter::new(file), Compression::fast());
            out.write_all(&map.to_bytes())?;
            out.finish()?.flush()?;
        }
        // Windows' rename does not replace an existing file on every filesystem, and a file that
        // failed to read back is exactly the one being replaced.
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path() != path && is_cache_name(key, &name) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// The request's map from the cache in `cache_dir` when it is there and whole, else baked and
/// cached (a cache that cannot be written still plays).
pub fn load_or_bake(
    req: &BakeRequest,
    cache_dir: Option<&Path>,
    progress: &dyn Fn(f32),
) -> Result<Baked> {
    let path = match cache_dir {
        Some(dir) => Some(cache_file(dir, req)?),
        None => None,
    };
    load_or_make(req.key, cache_dir, path.as_deref(), progress, || {
        bake(req, progress)
    })
}

/// [`load_or_bake`] with the baking passed in, so the cache's behaviour is tested without data.
fn load_or_make(
    key: &str,
    cache_dir: Option<&Path>,
    path: Option<&Path>,
    progress: &dyn Fn(f32),
    make: impl FnOnce() -> Result<BakeOutput>,
) -> Result<Baked> {
    if let Some(map) = path.and_then(read_cache) {
        progress(1.0);
        return Ok(Baked {
            map,
            from_cache: true,
            failures: Vec::new(),
            cache_error: None,
        });
    }
    let BakeOutput { map, failures } = make()?;
    let mut cache_error = None;
    if !failures.is_empty() {
        eprintln!(
            "nightfall-bake: {key}: {} files could not be read, so this map is not cached: {}",
            failures.len(),
            failures.join("; ")
        );
    } else if let (Some(dir), Some(path)) = (cache_dir, path) {
        if let Err(e) = write_cache(dir, key, path, &map) {
            let why = format!("writing {}: {e}", path.display());
            eprintln!("nightfall-bake: {key}: the map is not cached: {why}");
            cache_error = Some(why);
        }
    }
    Ok(Baked {
        map,
        from_cache: false,
        failures,
        cache_error,
    })
}

#[cfg(test)]
mod tests {
    use nightfall_sim::{Bounds, Grid, HeightMapBuilder, V2};

    use super::*;
    use crate::bake::PropPlacement;

    /// A fresh, empty folder of this test's own.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-bake-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn square(size: f32) -> Bounds {
        Bounds::new(vec![
            V2::new(0.0, 0.0),
            V2::new(size, 0.0),
            V2::new(size, size),
            V2::new(0.0, size),
        ])
        .unwrap()
    }

    fn small_map() -> HeightMap {
        let grid = Grid {
            origin: V2::ZERO,
            cell: CELL,
            nx: 20,
            ny: 20,
        };
        let mut b = HeightMapBuilder::flat(grid, 1.0);
        b.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 1.0, 3.0);
        b.finish()
    }

    fn output(map: HeightMap) -> BakeOutput {
        BakeOutput {
            map,
            failures: Vec::new(),
        }
    }

    /// Deflate `bytes` the way a cache file holds them.
    fn deflated(bytes: &[u8]) -> Vec<u8> {
        let mut out = DeflateEncoder::new(Vec::new(), Compression::fast());
        out.write_all(bytes).unwrap();
        out.finish().unwrap()
    }

    #[test]
    fn the_key_follows_the_data_the_bounds_and_the_props() {
        let data = scratch("key");
        std::fs::write(data.join("patch.MPQ"), b"one").unwrap();
        let (b10, b20) = (square(10.0), square(20.0));
        let prop = [PropPlacement {
            model: "World\\Crate.mdx".into(),
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 90.0, 0.0],
            scale: 1.0,
        }];
        let key = |bounds: &Bounds, props: &[PropPlacement]| {
            cache_key(&BakeRequest {
                data_dir: &data,
                map_id: 0,
                key: "test",
                bounds,
                props,
            })
            .unwrap()
        };
        let base = key(&b10, &[]);
        assert_eq!(base, key(&b10, &[]));
        assert_ne!(base, key(&b20, &[]));
        assert_ne!(base, key(&b10, &prop));
        std::fs::write(data.join("readme.txt"), b"not an archive").unwrap();
        assert_eq!(base, key(&b10, &[]));
        std::fs::write(data.join("patch.MPQ"), b"three!").unwrap();
        assert_ne!(base, key(&b10, &[]));
    }

    #[test]
    fn a_cache_reads_back_what_was_written_and_old_files_go() {
        let dir = scratch("roundtrip");
        let stale = dir.join("test-0000000000000001.bin");
        std::fs::write(&stale, b"old").unwrap();
        let other = dir.join("elwynn-0000000000000001.bin");
        std::fs::write(&other, b"another map").unwrap();
        // A map whose key begins with this one's, and files that merely look alike.
        let nested = dir.join("test-forest-0000000000000001.bin");
        std::fs::write(&nested, b"another map").unwrap();
        let unrelated = dir.join("test-notes.bin");
        std::fs::write(&unrelated, b"not a cache file").unwrap();
        let stale_tmp = dir.join("test-0000000000000002.4242.tmp");
        std::fs::write(&stale_tmp, b"a write that never finished").unwrap();
        let path = dir.join("test-00000000000000ff.bin");
        let map = small_map();
        write_cache(&dir, "test", &path, &map).unwrap();
        assert_eq!(read_cache(&path).unwrap(), map);
        assert!(!stale.exists());
        assert!(!stale_tmp.exists());
        assert!(other.exists());
        assert!(nested.exists());
        assert!(unrelated.exists());
        // This write's own temporary is gone too.
        assert!(std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn a_corrupt_cache_file_is_not_trusted() {
        let dir = scratch("corrupt");
        let garbage = dir.join("test-1.bin");
        std::fs::write(&garbage, b"this is not deflate").unwrap();
        assert!(read_cache(&garbage).is_none());
        let cut = dir.join("test-2.bin");
        write_cache(&dir, "test", &cut, &small_map()).unwrap();
        let bytes = std::fs::read(&cut).unwrap();
        std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        assert!(read_cache(&cut).is_none());
        assert!(read_cache(&dir.join("missing.bin")).is_none());
    }

    #[test]
    fn a_file_the_codec_rejects_is_baked_again_and_overwritten() {
        let map = small_map();
        let good = map.to_bytes();
        let mut wrong_version = good.clone();
        wrong_version[4..8].copy_from_slice(&99u32.to_le_bytes());
        let truncated = good[..good.len() - 1].to_vec();
        let mut foreign = good.clone();
        foreign[0] ^= 0xff;
        // Each of these inflates cleanly and fails only in `HeightMap::from_bytes`.
        for (name, bytes) in [
            ("wrong-version", wrong_version),
            ("truncated", truncated),
            ("foreign", foreign),
        ] {
            let dir = scratch(name);
            let path = dir.join("test-00000000000000aa.bin");
            std::fs::write(&path, deflated(&bytes)).unwrap();
            assert!(read_cache(&path).is_none(), "{name}");

            let baked = load_or_make("test", Some(&dir), Some(&path), &|_| {}, || {
                Ok(output(small_map()))
            })
            .unwrap();
            assert!(!baked.from_cache, "{name}");
            assert_eq!(baked.map, map, "{name}");
            // The bad file is gone: the next load is a cache hit on the fresh bake.
            assert_eq!(read_cache(&path).as_ref(), Some(&map), "{name}");
            let again = load_or_make("test", Some(&dir), Some(&path), &|_| {}, || {
                panic!("baked again though the cache was whole")
            })
            .unwrap();
            assert!(again.from_cache, "{name}");
        }
    }

    #[test]
    fn a_bake_that_could_not_read_everything_is_not_cached() {
        let dir = scratch("failed");
        let path = dir.join("test-00000000000000bb.bin");
        let failed = || {
            Ok(BakeOutput {
                map: small_map(),
                failures: vec!["tile test 1,2: damaged".into()],
            })
        };
        let first = load_or_make("test", Some(&dir), Some(&path), &|_| {}, failed).unwrap();
        assert!(!first.from_cache);
        assert_eq!(first.map, small_map());
        assert_eq!(first.failures.len(), 1);
        assert!(!path.exists(), "a map with unread files was cached");
        // The next launch bakes again, and a clean bake is kept.
        let second = load_or_make("test", Some(&dir), Some(&path), &|_| {}, || {
            Ok(output(small_map()))
        })
        .unwrap();
        assert!(!second.from_cache && second.failures.is_empty());
        assert!(path.exists());
    }

    #[test]
    fn a_cache_that_cannot_be_written_is_reported_and_the_map_still_plays() {
        let dir = scratch("unwritable");
        // A file where the cache folder should be.
        let blocked = dir.join("not-a-folder");
        std::fs::write(&blocked, b"x").unwrap();
        let path = blocked.join("test-00000000000000cc.bin");
        let baked = load_or_make("test", Some(&blocked), Some(&path), &|_| {}, || {
            Ok(output(small_map()))
        })
        .unwrap();
        assert_eq!(baked.map, small_map());
        assert!(baked.cache_error.is_some());
    }

    #[test]
    fn a_failed_write_leaves_no_temporary_behind() {
        let dir = scratch("tmp");
        // The destination is a folder, so the rename fails after the temporary was written.
        let path = dir.join("test-00000000000000dd.bin");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), b"x").unwrap();
        assert!(write_cache(&dir, "test", &path, &small_map()).is_err());
        assert!(std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn the_key_follows_a_symlinked_archive_to_its_target() {
        let data = scratch("symlink");
        let target = data.join("real.dat");
        std::fs::write(&target, b"one").unwrap();
        let link = data.join("patch.MPQ");
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_file(&target, &link).is_ok();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&target, &link).is_ok();
        if !linked {
            eprintln!("skipping: this platform or account cannot make symlinks");
            return;
        }
        let bounds = square(10.0);
        let key = || {
            cache_key(&BakeRequest {
                data_dir: &data,
                map_id: 0,
                key: "test",
                bounds: &bounds,
                props: &[],
            })
            .unwrap()
        };
        let before = key();
        std::fs::write(&target, b"another archive").unwrap();
        assert_ne!(before, key());
    }

    #[test]
    fn no_data_is_an_error_not_a_panic() {
        let data = scratch("nodata");
        let cache = scratch("nodata-cache");
        let bounds = square(10.0);
        let req = BakeRequest {
            data_dir: &data,
            map_id: 0,
            key: "test",
            bounds: &bounds,
            props: &[],
        };
        assert!(load_or_bake(&req, Some(&cache), &|_| {}).is_err());
    }
}
