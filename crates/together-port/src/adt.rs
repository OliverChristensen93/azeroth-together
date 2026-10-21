//! WotLK v18 ADT terrain tiles rewritten as the 1.12 tiles `benilla-adt` reads.
//!
//! Both are monolithic v18 files with the same top-level chunks and 128-byte `MCNK` header. What
//! differs, and what [`convert`] changes:
//!
//! - **Water.** WotLK keeps it in one top-level `MH2O` (per-chunk liquid instances, any sub-rect
//!   of the 8x8 cells); 1.12 in each `MCNK`'s `MCLQ`: one 0x324-byte block per liquid header bit
//!   (`0x04` river, `0x08` ocean, `0x10` magma, `0x20` slime), each cell's low nibble its kind
//!   (`0xf` dry). Instances become one block per kind; `MH2O` and its `MHDR` offset go.
//! - **Alpha maps.** A WotLK layer may be RLE-compressed (`MCLY` flag `0x200`) or, where the WDT's
//!   `MPHD` says big alpha (`0x4`), 8-bit 64x64. 1.12 reads 4-bit 2048-byte maps only, so every
//!   layer is unpacked and reduced to that.
//! - **What 1.12 lacks or reads otherwise** is dropped: `MCCV` vertex shading (flag `0x40`),
//!   `MCSE` sound emitters (28-byte records where 1.12's are 52), `MFBO` flight bounds and `MTXF`
//!   texture flags. `MCNR`'s 13 bytes of padding past its declared size are kept as 1.12 has them.
//!
//! Sub-chunks are found by the header's offsets, not in sequence (`MCNR`'s declared size does not
//! cover its padding), and written back in sequence with every offset and `MCIN` entry recomputed.

use anyhow::{bail, ensure, Context, Result};

/// The 1.12 class of a WotLK `LiquidType` row (its `Type` column, 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Liquid {
    Water,
    Ocean,
    Magma,
    Slime,
}

impl Liquid {
    pub fn from_type(t: u32) -> Self {
        match t {
            1 => Liquid::Ocean,
            2 => Liquid::Magma,
            3 => Liquid::Slime,
            _ => Liquid::Water,
        }
    }

    /// The `MCNK` header bit for one block of this kind.
    fn bit(self) -> u32 {
        match self {
            Liquid::Water => 0x04,
            Liquid::Ocean => 0x08,
            Liquid::Magma => 0x10,
            Liquid::Slime => 0x20,
        }
    }

    /// The cell nibble 1.12's own tiles use: 4 for rivers and lakes, 1 ocean, 6 magma, 3 slime.
    fn nibble(self) -> u8 {
        match self {
            Liquid::Water => 4,
            Liquid::Ocean => 1,
            Liquid::Magma => 6,
            Liquid::Slime => 3,
        }
    }

    /// Depth bytes per yard where a block has no depth of its own: what 1.12's rivers (8.96)
    /// and sea (1.72) are authored at.
    fn depth_per_yard(self) -> f32 {
        match self {
            Liquid::Ocean => 1.72,
            _ => 8.96,
        }
    }
}

/// Bytes per MCLQ block.
pub const MCLQ_BLOCK: usize = 0x324;
const MCNK_HEADER: usize = 128;
/// 4-bit alpha: 64x64 nibbles.
const SMALL_ALPHA: usize = 2048;
const BIG_ALPHA: usize = 4096;
/// MCNR's 145 normals and the 13 padding bytes after them.
const MCNR_WITH_PAD: usize = 448;

fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(b, at))
}

fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn push_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend(tag.iter().rev());
    out.extend((data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

/// Top-level chunks: tag as read (`MVER`), offset of the chunk's magic, payload.
fn chunks(b: &[u8]) -> Vec<([u8; 4], usize, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 8 <= b.len() {
        let tag = [b[i + 3], b[i + 2], b[i + 1], b[i]];
        let n = u32_at(b, i + 4) as usize;
        let end = (i + 8 + n).min(b.len());
        out.push((tag, i, &b[i + 8..end]));
        i = end;
    }
    out
}

/// The WDT's `MPHD` flags.
pub fn wdt_flags(wdt: &[u8]) -> u32 {
    chunks(wdt).into_iter().find(|(t, _, _)| t == b"MPHD").map_or(0, |(_, _, d)| u32_at(d, 0))
}

/// Whether the WDT's tiles store uncompressed alpha as 8-bit maps (`MPHD` `0x4`; `0x80`, height
/// texturing, implies it too).
pub fn big_alpha(wdt: &[u8]) -> bool {
    wdt_flags(wdt) & 0x84 != 0
}

/// The `(x, y)` of every tile the WDT's `MAIN` marks (`x` is the file name's first number).
pub fn wdt_tiles(wdt: &[u8]) -> Vec<(u32, u32)> {
    let Some((_, _, main)) = chunks(wdt).into_iter().find(|(t, _, _)| t == b"MAIN") else {
        return Vec::new();
    };
    // MAIN is row-major by y: entry `y * 64 + x`.
    (0..64 * 64u32).filter(|&i| u32_at(main, i as usize * 8) & 1 != 0).map(|i| (i % 64, i / 64)).collect()
}

/// Mark `tiles` in the WDT's `MAIN`; true if any was not marked.
pub fn mark_tiles(wdt: &mut [u8], tiles: &[(u32, u32)]) -> Result<bool> {
    let (at, len) = chunks(wdt)
        .into_iter()
        .find(|(t, _, _)| t == b"MAIN")
        .map(|(_, at, d)| (at, d.len()))
        .context("a WDT without MAIN")?;
    ensure!(len >= 64 * 64 * 8, "a short MAIN");
    let mut changed = false;
    for &(x, y) in tiles {
        let o = at + 8 + (y * 64 + x) as usize * 8;
        let f = u32_at(wdt, o);
        if f & 1 == 0 {
            put_u32(wdt, o, f | 1);
            changed = true;
        }
    }
    Ok(changed)
}

/// A WotLK WDT for a map 1.12 has none of: its `MPHD` flags cleared but bit 0 (a map of one WMO,
/// which 1.12 reads too) and only `tiles` marked.
pub fn new_wdt(wotlk: &[u8], tiles: &[(u32, u32)]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for (tag, _, d) in chunks(wotlk) {
        match &tag {
            b"MPHD" => {
                let mut d = d.to_vec();
                let keep = u32_at(&d, 0) & 1;
                put_u32(&mut d, 0, keep);
                push_chunk(&mut out, &tag, &d);
            }
            b"MAIN" => push_chunk(&mut out, &tag, &vec![0u8; d.len()]),
            _ => push_chunk(&mut out, &tag, d),
        }
    }
    mark_tiles(&mut out, tiles)?;
    Ok(out)
}

/// The area ids the tile's chunks carry (`MCNK` header `+0x34`).
pub fn area_ids(adt: &[u8]) -> Vec<u32> {
    let mut ids: Vec<u32> =
        chunks(adt).into_iter().filter(|(t, _, _)| t == b"MCNK").map(|(_, _, d)| u32_at(d, 0x34)).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// The NUL-separated names of the tile's `MMDX`, `MWMO` and `MTEX`: what it places and paints.
pub fn named(adt: &[u8]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut models, mut wmos, mut textures) = (Vec::new(), Vec::new(), Vec::new());
    for (tag, _, d) in chunks(adt) {
        let into = match &tag {
            b"MMDX" => &mut models,
            b"MWMO" => &mut wmos,
            b"MTEX" => &mut textures,
            _ => continue,
        };
        into.extend(d.split(|&c| c == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()));
    }
    (models, wmos, textures)
}

/// One liquid instance of one chunk, read out of `MH2O`.
struct Instance {
    kind: Liquid,
    min: f32,
    max: f32,
    /// The covered rect, in cells: x, y, width, height.
    rect: (usize, usize, usize, usize),
    /// Per covered cell, row-major over the rect.
    exists: Vec<bool>,
    /// Per vertex of the rect ((w+1)*(h+1), row-major), where the instance has them.
    heights: Option<Vec<f32>>,
    depths: Option<Vec<u8>>,
}

/// The instances of each of the 256 chunks. Offsets in `MH2O` count from its payload.
fn read_mh2o(d: &[u8], kind_of: &dyn Fn(u16) -> Liquid) -> Result<Vec<Vec<Instance>>> {
    ensure!(d.len() >= 256 * 12, "a short MH2O");
    let mut out = Vec::with_capacity(256);
    for c in 0..256 {
        let (ofs, count) = (u32_at(d, c * 12) as usize, u32_at(d, c * 12 + 4) as usize);
        let mut list = Vec::new();
        for k in 0..count {
            let i = ofs + k * 24;
            ensure!(i + 24 <= d.len(), "MH2O instance past the chunk");
            let ty = u16::from_le_bytes([d[i], d[i + 1]]);
            let format = u16::from_le_bytes([d[i + 2], d[i + 3]]);
            let (min, max) = (f32_at(d, i + 4), f32_at(d, i + 8));
            // Format 2 (depth only: the open sea) always covers the whole chunk.
            let rect = if format == 2 {
                (0, 0, 8, 8)
            } else {
                (d[i + 12] as usize, d[i + 13] as usize, d[i + 14] as usize, d[i + 15] as usize)
            };
            let (x, y, w, h) = rect;
            ensure!(x + w <= 8 && y + h <= 8, "MH2O instance rect {rect:?} outside its chunk");
            let (bits, data) = (u32_at(d, i + 16) as usize, u32_at(d, i + 20) as usize);
            let exists = (0..w * h)
                .map(|n| bits == 0 || d.get(bits + n / 8).is_some_and(|b| b >> (n % 8) & 1 != 0))
                .collect();
            let verts = (w + 1) * (h + 1);
            let floats = |at: usize| -> Option<Vec<f32>> {
                (at + verts * 4 <= d.len()).then(|| (0..verts).map(|n| f32_at(d, at + n * 4)).collect())
            };
            let bytes = |at: usize| -> Option<Vec<u8>> { d.get(at..at + verts).map(<[u8]>::to_vec) };
            // 0: heights, depths; 1: heights, UVs; 2: depths; 3: heights, UVs, depths.
            let (heights, depths) = match (data, format) {
                (0, _) => (None, None),
                (_, 0) => (floats(data), bytes(data + verts * 4)),
                (_, 1) => (floats(data), None),
                (_, 2) => (None, bytes(data)),
                (_, 3) => (floats(data), bytes(data + verts * 8)),
                (_, f) => bail!("MH2O vertex format {f}"),
            };
            list.push(Instance { kind: kind_of(ty), min, max, rect, exists, heights, depths });
        }
        out.push(list);
    }
    Ok(out)
}
/// One MCLQ block of `kind` from the chunk's instances of it. `terrain` is the chunk's absolute
/// outer-grid heights (9x9), for a depth where the instance has none; `origin` the chunk's
/// global vertex `(column, row)`, for magma's world-continuous texture coordinates.
fn mclq_block(kind: Liquid, instances: &[&Instance], terrain: &[f32; 81], origin: (u32, u32)) -> Vec<u8> {
    let mut heights: [Option<f32>; 81] = [None; 81];
    let mut depths: [Option<u8>; 81] = [None; 81];
    let mut wet = [false; 64];
    let (mut min, mut max) = (f32::MAX, f32::MIN);
    for inst in instances {
        min = min.min(inst.min);
        max = max.max(inst.max);
        let (x0, y0, w, h) = inst.rect;
        for r in 0..h {
            for c in 0..w {
                if inst.exists[r * w + c] {
                    wet[(y0 + r) * 8 + x0 + c] = true;
                }
            }
        }
        for r in 0..=h {
            for c in 0..=w {
                let (n, v) = (r * (w + 1) + c, (y0 + r) * 9 + x0 + c);
                heights[v] = Some(inst.heights.as_ref().map_or(inst.min, |hs| hs[n]));
                if let Some(ds) = &inst.depths {
                    depths[v] = Some(ds[n]);
                }
            }
        }
    }
    let mut b = vec![0u8; MCLQ_BLOCK];
    b[0..4].copy_from_slice(&min.to_le_bytes());
    b[4..8].copy_from_slice(&max.to_le_bytes());
    for v in 0..81 {
        let h = heights[v].unwrap_or(min);
        let o = 8 + v * 8;
        match kind {
            Liquid::Water | Liquid::Ocean => {
                b[o] = depths[v]
                    .unwrap_or_else(|| ((h - terrain[v]) * kind.depth_per_yard()).clamp(0.0, 255.0) as u8);
            }
            Liquid::Magma | Liquid::Slime => {
                // 21 per vertex is about a quarter texture (1.12 scales by 3/256), and 3072
                // vertices are a whole number of textures, so the wrap is seamless.
                let (col, row) = (origin.0 + (v % 9) as u32, origin.1 + (v / 9) as u32);
                let s = (col % 3072 * 21) as u16;
                let t = (row % 3072 * 21) as u16;
                b[o..o + 2].copy_from_slice(&s.to_le_bytes());
                b[o + 2..o + 4].copy_from_slice(&t.to_le_bytes());
            }
        }
        b[o + 4..o + 8].copy_from_slice(&h.to_le_bytes());
    }
    let flags = 8 + 81 * 8;
    for (i, &w) in wet.iter().enumerate() {
        b[flags + i] = if w { kind.nibble() } else { 0x0f };
    }
    // The flow count and two flow records after the flags stay zero.
    b
}

/// RLE alpha (`MCLY` `0x200`) to 4096 8-bit values: a token's bit 7 is fill (one value repeated)
/// or copy, its low 7 bits the count.
pub fn unpack_rle(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(BIG_ALPHA);
    let mut i = 0;
    while out.len() < BIG_ALPHA && i < src.len() {
        let (token, count) = (src[i], (src[i] & 0x7f) as usize);
        i += 1;
        if token & 0x80 != 0 {
            let v = src.get(i).copied().unwrap_or(0);
            i += 1;
            out.extend(std::iter::repeat_n(v, count));
        } else {
            let end = (i + count).min(src.len());
            out.extend_from_slice(&src[i..end]);
            i = end;
        }
    }
    out.resize(BIG_ALPHA, 0);
    out
}

/// 4096 8-bit values to 1.12's 2048 bytes: rounded to 4 bits, the even texel in the low nibble.
pub fn to_small(big: &[u8]) -> Vec<u8> {
    let q = |v: u8| ((u32::from(v) * 15 + 127) / 255) as u8;
    big.as_chunks::<2>().0.iter().map(|p| q(p[0]) | q(p[1]) << 4).collect()
}

/// The payload of the sub-chunk a header offset names (from the `MCNK` magic, or its data).
fn sub<'a>(data: &'a [u8], ofs: u32, tag: &[u8; 4]) -> Option<&'a [u8]> {
    if ofs == 0 {
        return None;
    }
    let rev: [u8; 4] = [tag[3], tag[2], tag[1], tag[0]];
    let ofs = ofs as usize;
    [ofs.checked_sub(8), Some(ofs)].into_iter().flatten().find_map(|at| {
        (data.get(at..at + 4) == Some(rev.as_slice())).then(|| {
            let n = u32_at(data, at + 4) as usize;
            &data[at + 8..(at + 8 + n).min(data.len())]
        })
    })
}

/// One WotLK `MCNK` payload as 1.12's.
fn convert_mcnk(data: &[u8], liquids: &[Instance], big: bool, tile: (u32, u32)) -> Result<Vec<u8>> {
    ensure!(data.len() >= MCNK_HEADER, "a short MCNK");
    let mut h = data[..MCNK_HEADER].to_vec();
    let (ix, iy) = (u32_at(&h, 0x04), u32_at(&h, 0x08));
    let mut body: Vec<u8> = Vec::new();
    // Where the next sub-chunk's magic lands, counted from the MCNK magic.
    let at = |body: &Vec<u8>| (8 + MCNK_HEADER + body.len()) as u32;

    let mcvt = sub(data, u32_at(&h, 0x14), b"MCVT").context("no MCVT")?;
    put_u32(&mut h, 0x14, at(&body));
    push_chunk(&mut body, b"MCVT", mcvt);

    if let Some(mcnr) = sub(data, u32_at(&h, 0x18), b"MCNR") {
        put_u32(&mut h, 0x18, at(&body));
        let mut n = mcnr.to_vec();
        n.resize(MCNR_WITH_PAD, 0);
        push_chunk(&mut body, b"MCNR", &n[..435]);
        body.extend_from_slice(&n[435..]);
    }

    let mcly = sub(data, u32_at(&h, 0x1C), b"MCLY").unwrap_or(&[]);
    let mcal = sub(data, u32_at(&h, 0x24), b"MCAL").unwrap_or(&[]);
    let mut layers = mcly.to_vec();
    let mut alpha = Vec::new();
    for (i, l) in layers.as_chunks_mut::<16>().0.iter_mut().enumerate() {
        let (flags, ofs) = (u32_at(l, 4), u32_at(l, 8) as usize);
        put_u32(l, 4, flags & !0x200);
        put_u32(l, 8, alpha.len() as u32);
        if i == 0 || flags & 0x100 == 0 {
            continue;
        }
        let src = mcal.get(ofs..).unwrap_or(&[]);
        let small = if flags & 0x200 != 0 {
            to_small(&unpack_rle(src))
        } else if big {
            let mut b = src[..src.len().min(BIG_ALPHA)].to_vec();
            b.resize(BIG_ALPHA, 0);
            to_small(&b)
        } else {
            let mut s = src[..src.len().min(SMALL_ALPHA)].to_vec();
            s.resize(SMALL_ALPHA, 0);
            s
        };
        alpha.extend(small);
    }
    put_u32(&mut h, 0x1C, at(&body));
    push_chunk(&mut body, b"MCLY", &layers);

    let mcrf = sub(data, u32_at(&h, 0x20), b"MCRF").unwrap_or(&[]);
    put_u32(&mut h, 0x20, at(&body));
    push_chunk(&mut body, b"MCRF", mcrf);

    match sub(data, u32_at(&h, 0x2C), b"MCSH") {
        Some(sh) => {
            put_u32(&mut h, 0x2C, at(&body));
            put_u32(&mut h, 0x30, sh.len() as u32);
            push_chunk(&mut body, b"MCSH", sh);
        }
        None => {
            put_u32(&mut h, 0x2C, 0);
            put_u32(&mut h, 0x30, 0);
        }
    }

    put_u32(&mut h, 0x24, at(&body));
    put_u32(&mut h, 0x28, alpha.len() as u32 + 8);
    push_chunk(&mut body, b"MCAL", &alpha);

    // No sound emitters (their records differ), no MCCV.
    put_u32(&mut h, 0x58, 0);
    put_u32(&mut h, 0x5C, 0);
    for o in [0x74, 0x78, 0x7C] {
        put_u32(&mut h, o, 0);
    }

    // Liquid: one block per kind, in header-bit order, as 1.12 reads them.
    let mut flags = u32_at(&h, 0) & 0x3; // shadow map, impassable
    let mut kinds: Vec<Liquid> = liquids.iter().map(|l| l.kind).collect();
    kinds.sort();
    kinds.dedup();
    if kinds.is_empty() {
        put_u32(&mut h, 0x60, 0);
        put_u32(&mut h, 0x64, 0);
    } else {
        let base = f32_at(&h, 0x70);
        let mut terrain = [0f32; 81];
        for (v, t) in terrain.iter_mut().enumerate() {
            *t = base + f32_at(mcvt, ((v / 9) * 17 + v % 9) * 4);
        }
        let origin = (tile.0 * 128 + ix * 8, tile.1 * 128 + iy * 8);
        let mut mclq = Vec::new();
        for k in kinds {
            flags |= k.bit();
            let of_kind: Vec<&Instance> = liquids.iter().filter(|l| l.kind == k).collect();
            mclq.extend(mclq_block(k, &of_kind, &terrain, origin));
        }
        put_u32(&mut h, 0x60, at(&body));
        put_u32(&mut h, 0x64, mclq.len() as u32 + 8);
        // 1.12's MCLQ declares size 0; the header's size is the one read.
        body.extend(b"QLCM");
        body.extend(0u32.to_le_bytes());
        body.extend(mclq);
    }
    put_u32(&mut h, 0, flags);

    h.extend(body);
    Ok(h)
}

/// Rewrite a WotLK ADT for 1.12. `tile` is its `(x, y)`; `big` the WDT's [`big_alpha`];
/// `kind_of` a WotLK `LiquidType` id's class.
pub fn convert(adt: &[u8], tile: (u32, u32), big: bool, kind_of: &dyn Fn(u16) -> Liquid) -> Result<Vec<u8>> {
    let parts = chunks(adt);
    let liquids = match parts.iter().find(|(t, _, _)| t == b"MH2O") {
        Some((_, _, d)) => read_mh2o(d, kind_of)?,
        None => (0..256).map(|_| Vec::new()).collect(),
    };
    let mut out = Vec::with_capacity(adt.len());
    // MHDR offsets count from its payload; MCIN's from the file's start.
    let (mut mhdr_at, mut mcin_at, mut mcnk) = (None, None, 0usize);
    let mut offsets = [0u32; 16];
    let mut mcin = vec![0u8; 256 * 16];
    for (tag, _, d) in &parts {
        let here = out.len();
        match tag {
            b"MHDR" => {
                mhdr_at = Some(here);
                push_chunk(&mut out, tag, &[0u8; 64]);
            }
            b"MCIN" => {
                mcin_at = Some(here);
                push_chunk(&mut out, tag, &mcin);
            }
            b"MH2O" | b"MFBO" | b"MTXF" => {}
            b"MCNK" => {
                ensure!(mcnk < 256, "more than 256 MCNKs");
                let c = convert_mcnk(d, &liquids[mcnk], big, tile).with_context(|| format!("MCNK {mcnk}"))?;
                put_u32(&mut mcin, mcnk * 16, here as u32);
                put_u32(&mut mcin, mcnk * 16 + 4, c.len() as u32 + 8);
                push_chunk(&mut out, tag, &c);
                mcnk += 1;
            }
            _ => push_chunk(&mut out, tag, d),
        }
        let slot = match tag {
            b"MCIN" => 1,
            b"MTEX" => 2,
            b"MMDX" => 3,
            b"MMID" => 4,
            b"MWMO" => 5,
            b"MWID" => 6,
            b"MDDF" => 7,
            b"MODF" => 8,
            _ => continue,
        };
        offsets[slot] = here as u32;
    }
    ensure!(mcnk == 256, "{mcnk} MCNKs, not 256");
    let mhdr_at = mhdr_at.context("no MHDR")?;
    let base = mhdr_at as u32 + 8;
    for (slot, &o) in offsets.iter().enumerate().skip(1) {
        if o != 0 {
            put_u32(&mut out, mhdr_at + 8 + slot * 4, o - base);
        }
    }
    let mcin_at = mcin_at.context("no MCIN")?;
    out[mcin_at + 8..mcin_at + 8 + mcin.len()].copy_from_slice(&mcin);
    Ok(out)
}

/// Read a converted tile back with `benilla-adt`, as the client will: 256 chunks with heights,
/// normals, a 4-bit alpha map per layer that has one, and one liquid block per liquid bit; then
/// mesh it with the client's terrain mesher. The number of liquid surfaces it meshes.
pub fn check(adt: &[u8]) -> Result<usize> {
    let benilla_adt::ParsedAdt::Root(root) =
        benilla_adt::parse_adt(&mut std::io::Cursor::new(adt)).map_err(|e| anyhow::anyhow!("benilla-adt: {e}"))?;
    ensure!(root.mcnk_chunks.len() == 256, "{} chunks", root.mcnk_chunks.len());
    for (i, c) in root.mcnk_chunks.iter().enumerate() {
        ensure!(c.heights.as_ref().is_some_and(|h| h.heights.len() == 145), "chunk {i}: no heights");
        ensure!(c.normals.as_ref().is_some_and(|n| n.normals.len() == 145), "chunk {i}: no normals");
        let maps = c.layers.as_ref().map_or(0, |l| l.layers.iter().skip(1).filter(|l| l.flags.value & 0x100 != 0).count());
        let alpha = c.alpha.as_ref().map_or(0, |a| a.data.len());
        ensure!(alpha >= maps * SMALL_ALPHA, "chunk {i}: {alpha} alpha bytes for {maps} alpha maps");
        let bits = (c.header.flags & 0x3c).count_ones() as usize;
        ensure!(c.liquids.len() == bits, "chunk {i}: {} liquid blocks for {bits} bits", c.liquids.len());
    }
    let mesh = benilla_formats::adt_to_tile_mesh(adt).context("the terrain mesher")?;
    ensure!(mesh.chunks.len() == 256, "{} chunks meshed", mesh.chunks.len());
    Ok(mesh.chunks.iter().map(|c| c.liquids.len()).sum())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A WotLK-shaped MCNK: header, MCVT, MCNR (declared 435, padded to 448), MCLY with `layers`,
    /// MCRF, MCSH, MCAL, MCCV and MCSE, offsets from the MCNK magic.
    fn wotlk_mcnk(ix: u32, iy: u32, layers: &[(u32, u32)], mcal: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; MCNK_HEADER];
        put_u32(&mut h, 0, 0x8041); // MCSH, MCCV, do-not-fix
        put_u32(&mut h, 4, ix);
        put_u32(&mut h, 8, iy);
        put_u32(&mut h, 0x0C, layers.len() as u32);
        put_u32(&mut h, 0x34, 12);
        h[0x70..0x74].copy_from_slice(&100f32.to_le_bytes());
        let mut body = Vec::new();
        let at = |b: &Vec<u8>| (8 + MCNK_HEADER + b.len()) as u32;
        put_u32(&mut h, 0x14, at(&body));
        push_chunk(&mut body, b"MCVT", &[0u8; 145 * 4]);
        put_u32(&mut h, 0x18, at(&body));
        push_chunk(&mut body, b"MCNR", &[1u8; 435]);
        body.extend([0u8; 13]);
        put_u32(&mut h, 0x1C, at(&body));
        let ly: Vec<u8> = layers.iter().flat_map(|&(f, o)| [7u32, f, o, 0]).flat_map(u32::to_le_bytes).collect();
        push_chunk(&mut body, b"MCLY", &ly);
        put_u32(&mut h, 0x20, at(&body));
        push_chunk(&mut body, b"MCRF", &[]);
        put_u32(&mut h, 0x2C, at(&body));
        put_u32(&mut h, 0x30, 512);
        push_chunk(&mut body, b"MCSH", &[0u8; 512]);
        put_u32(&mut h, 0x24, at(&body));
        put_u32(&mut h, 0x28, mcal.len() as u32 + 8);
        push_chunk(&mut body, b"MCAL", mcal);
        put_u32(&mut h, 0x74, at(&body));
        push_chunk(&mut body, b"MCCV", &[0x7f; 145 * 4]);
        put_u32(&mut h, 0x58, at(&body));
        put_u32(&mut h, 0x5C, 1);
        push_chunk(&mut body, b"MCSE", &[0u8; 28]);
        h.extend(body);
        h
    }

    /// A tile: MVER, MHDR, MCIN, MTEX, MH2O (if any), 256 MCNKs; chunk 0 is `chunk0`.
    fn wotlk_tile(chunk0: Vec<u8>, mh2o: Option<Vec<u8>>) -> Vec<u8> {
        let mut b = Vec::new();
        push_chunk(&mut b, b"MVER", &18u32.to_le_bytes());
        push_chunk(&mut b, b"MHDR", &[0u8; 64]);
        push_chunk(&mut b, b"MCIN", &[0u8; 4096]);
        push_chunk(&mut b, b"MTEX", b"Tileset\\Grass.blp\0");
        if let Some(m) = mh2o {
            push_chunk(&mut b, b"MH2O", &m);
        }
        push_chunk(&mut b, b"MCNK", &chunk0);
        for i in 1..256 {
            push_chunk(&mut b, b"MCNK", &wotlk_mcnk(i % 16, i / 16, &[(0, 0)], &[]));
        }
        b
    }

    fn parse(b: &[u8]) -> Box<benilla_adt::RootAdt> {
        check(b).expect("checks");
        let benilla_adt::ParsedAdt::Root(r) = benilla_adt::parse_adt(&mut Cursor::new(b)).expect("benilla-adt parses");
        r
    }

    #[test]
    fn rle_alpha_unpacks_and_packs_to_four_bits() {
        // Fill 100 x 255, copy 0, 17, 34, then fill the rest with 0.
        let mut rle = vec![0x80 | 100, 255, 3, 0, 17, 34];
        let mut left = BIG_ALPHA - 103;
        while left > 0 {
            let n = left.min(127);
            rle.extend([0x80 | n as u8, 0]);
            left -= n;
        }
        let big = unpack_rle(&rle);
        assert_eq!(big.len(), BIG_ALPHA);
        assert!(big[..100].iter().all(|&v| v == 255));
        assert_eq!(&big[100..103], &[0, 17, 34]);
        let small = to_small(&big);
        assert_eq!(small.len(), SMALL_ALPHA);
        assert_eq!(small[0], 0xff);
        assert_eq!(small[50], 1 << 4, "texel 100 is 0, 101 is 17, so 1");
        assert_eq!(small[51], 2, "texel 102 is 34, so 2; 103 is 0");
    }

    #[test]
    fn a_compressed_layer_becomes_a_small_map_benilla_reads() {
        let rle: Vec<u8> = std::iter::repeat_n([0x80 | 64, 255], 64).flatten().collect();
        let tile = wotlk_tile(wotlk_mcnk(0, 0, &[(0, 0), (0x300, 0)], &rle), None);
        let out = convert(&tile, (32, 40), false, &|_| Liquid::Water).unwrap();
        let root = parse(&out);
        assert_eq!(root.textures, vec!["Tileset\\Grass.blp"]);
        let c = &root.mcnk_chunks[0];
        let ly = &c.layers.as_ref().unwrap().layers;
        assert_eq!(ly[1].flags.value, 0x100, "the compressed flag is cleared");
        assert_eq!(c.alpha.as_ref().unwrap().data.len(), SMALL_ALPHA);
        assert!(c.alpha.as_ref().unwrap().data.iter().all(|&b| b == 0xff));
        assert!(c.shadow.is_some());
        assert_eq!(c.header.area_id, 12);
        assert_eq!(c.header.flags, 0x1, "MCSH kept, MCCV and do-not-fix gone");
        // MCIN points at every MCNK, and MHDR at MCIN.
        let parts = chunks(&out);
        let (_, mcin_at, mcin) = parts.iter().find(|(t, _, _)| t == b"MCIN").unwrap();
        let (_, mhdr_at, mhdr) = parts.iter().find(|(t, _, _)| t == b"MHDR").unwrap();
        assert_eq!(u32_at(mhdr, 4) as usize + mhdr_at + 8, *mcin_at);
        for (tag, at, _) in &parts {
            if tag == b"MCNK" {
                assert!(mcin.as_chunks::<16>().0.iter().any(|e| u32_at(e, 0) as usize == *at));
            }
        }
    }

    #[test]
    fn a_big_alpha_layer_is_reduced() {
        let tile = wotlk_tile(wotlk_mcnk(0, 0, &[(0, 0), (0x100, 0)], &[0x88; BIG_ALPHA]), None);
        let root = parse(&convert(&tile, (0, 0), true, &|_| Liquid::Water).unwrap());
        let a = &root.mcnk_chunks[0].alpha.as_ref().unwrap().data;
        assert_eq!(a.len(), SMALL_ALPHA);
        assert!(a.iter().all(|&b| b == 0x88), "0x88 is 8 in four bits");
    }

    /// An MH2O with two instances on chunk 0: water (format 0) over cells x 2..4, y 1..3 with
    /// cell 1 of the rect dry, and the open sea (format 2) over all of it.
    fn mh2o() -> Vec<u8> {
        let mut d = vec![0u8; 256 * 12];
        let inst_at = d.len();
        put_u32(&mut d, 0, inst_at as u32);
        put_u32(&mut d, 4, 2);
        d.resize(inst_at + 48, 0);
        let bits_at = d.len();
        d.push(0b1111_1101);
        let verts_at = d.len();
        for n in 0..9 {
            d.extend((50.0f32 + n as f32).to_le_bytes());
        }
        d.extend([9u8; 9]);
        let ocean_depth_at = d.len();
        d.extend([200u8; 81]);
        let w = &mut d[inst_at..inst_at + 24];
        w[0..2].copy_from_slice(&5u16.to_le_bytes()); // Slow Water
        w[4..8].copy_from_slice(&50f32.to_le_bytes());
        w[8..12].copy_from_slice(&58f32.to_le_bytes());
        w[12..16].copy_from_slice(&[2, 1, 2, 2]);
        w[16..20].copy_from_slice(&(bits_at as u32).to_le_bytes());
        w[20..24].copy_from_slice(&(verts_at as u32).to_le_bytes());
        let o = &mut d[inst_at + 24..inst_at + 48];
        o[0..2].copy_from_slice(&2u16.to_le_bytes()); // Ocean
        o[2..4].copy_from_slice(&2u16.to_le_bytes());
        o[20..24].copy_from_slice(&(ocean_depth_at as u32).to_le_bytes());
        d
    }

    #[test]
    fn mh2o_instances_become_mclq_blocks() {
        let tile = wotlk_tile(wotlk_mcnk(0, 0, &[(0, 0)], &[]), Some(mh2o()));
        let kind = |t: u16| if t == 2 { Liquid::Ocean } else { Liquid::Water };
        let out = convert(&tile, (32, 40), false, &kind).unwrap();
        assert_eq!(check(&out).unwrap(), 2, "the mesher draws the river and the sea");
        assert!(!chunks(&out).iter().any(|(t, _, _)| t == b"MH2O"), "MH2O is gone");
        let root = parse(&out);
        let c = &root.mcnk_chunks[0];
        assert_eq!(c.header.flags & 0x3c, 0x0c, "river and ocean bits");
        assert_eq!(c.liquids.len(), 2);
        let (river, sea) = (&c.liquids[0], &c.liquids[1]);
        assert_eq!(river.tile_flags[8 + 2], 4);
        assert_eq!(river.tile_flags[8 + 3], 0x0f, "the rect's dry cell");
        assert_eq!(river.tile_flags[16 + 2], 4);
        assert_eq!(river.tile_flags[16 + 3], 4);
        assert_eq!(river.tile_flags[0], 0x0f);
        assert_eq!(river.vertices[9 + 2].height, 50.0, "the rect's first vertex");
        assert_eq!(river.vertices[2 * 9 + 3].height, 54.0, "the rect's (row 1, col 1)");
        assert_eq!(river.vertices[9 + 2].depth_byte(), 9);
        assert_eq!((river.min_height, river.max_height), (50.0, 58.0));
        assert!(sea.tile_flags.iter().all(|&f| f == 1), "the sea covers the chunk");
        assert_eq!(sea.vertices[40].depth_byte(), 200);
        assert_eq!(sea.vertices[40].height, 0.0);
        assert!(root.mcnk_chunks[1].liquids.is_empty(), "the rest stay dry");
    }

    #[test]
    fn wdt_tiles_are_marked_and_new_wdts_cleared() {
        let mut w = Vec::new();
        push_chunk(&mut w, b"MVER", &18u32.to_le_bytes());
        push_chunk(&mut w, b"MPHD", &[0x86, 0, 0, 0, 0, 0, 0, 0]);
        let mut main = vec![0u8; 64 * 64 * 8];
        put_u32(&mut main, (40 * 64 + 31) * 8, 1);
        push_chunk(&mut w, b"MAIN", &main);
        assert!(big_alpha(&w));
        assert_eq!(wdt_tiles(&w), vec![(31, 40)]);
        let fresh = new_wdt(&w, &[(32, 40)]).unwrap();
        assert_eq!(wdt_flags(&fresh), 0);
        assert_eq!(wdt_tiles(&fresh), vec![(32, 40)]);
        assert!(mark_tiles(&mut w, &[(32, 40)]).unwrap());
        assert!(!mark_tiles(&mut w, &[(31, 40)]).unwrap());
        assert_eq!(wdt_tiles(&w), vec![(31, 40), (32, 40)]);
    }
}