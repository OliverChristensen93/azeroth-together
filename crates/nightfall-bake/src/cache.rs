//! The bake's cache: `<dir>/<key>-<hash>.bin`, deflated. The hash covers what the bake reads
//! (every MPQ's name, size and time, so a patched or swapped install bakes again), what it is
//! asked (the map, the bounds, the props) and [`BAKE_VERSION`]. A file that does not read back
//! whole (damaged, cut short, or from another codec version) is baked again and overwritten; older
//! files for the same map are removed once a new one is written.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use nightfall_sim::{HeightMap, CELL};

use crate::bake::{bake, BakeRequest};

/// Bumped whenever the bake would make a different map from the same data.
pub const BAKE_VERSION: u32 = 1;

/// A height map, and whether it came from the cache.
pub struct Baked {
    pub map: HeightMap,
    pub from_cache: bool,
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
        let meta = entry.metadata()?;
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

/// Write the map to `path` (through a temporary file, so a crash never leaves half a map under
/// the real name), then remove this map's older cache files in `dir`.
pub(crate) fn write_cache(
    dir: &Path,
    key: &str,
    path: &Path,
    map: &HeightMap,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    {
        let file = std::fs::File::create(&tmp)?;
        let mut out = DeflateEncoder::new(BufWriter::new(file), Compression::fast());
        out.write_all(&map.to_bytes())?;
        out.finish()?.flush()?;
    }
    // Windows' rename does not replace an existing file on every filesystem, and a file that
    // failed to read back is exactly the one being replaced.
    let _ = std::fs::remove_file(path);
    std::fs::rename(&tmp, path)?;
    let prefix = format!("{key}-");
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path() != path && name.starts_with(&prefix) && name.ends_with(".bin") {
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
    make: impl FnOnce() -> Result<HeightMap>,
) -> Result<Baked> {
    if let Some(map) = path.and_then(read_cache) {
        progress(1.0);
        return Ok(Baked {
            map,
            from_cache: true,
        });
    }
    let map = make()?;
    if let (Some(dir), Some(path)) = (cache_dir, path) {
        let _ = write_cache(dir, key, path, &map);
    }
    Ok(Baked {
        map,
        from_cache: false,
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
        let path = dir.join("test-00000000000000ff.bin");
        let map = small_map();
        write_cache(&dir, "test", &path, &map).unwrap();
        assert_eq!(read_cache(&path).unwrap(), map);
        assert!(!stale.exists());
        assert!(other.exists());
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

            let baked =
                load_or_make("test", Some(&dir), Some(&path), &|_| {}, || Ok(small_map())).unwrap();
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
