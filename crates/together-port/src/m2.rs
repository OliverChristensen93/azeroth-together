//! An M2 model from WotLK (MD20 v264: skin profiles in `.skin` files, some animations' keys in
//! `.anim` files, every track an array of per-sequence key arrays) rewritten as 1.12's v256 (skin
//! profiles embedded, one key array per track on a shared timeline that each sequence owns a
//! window of, `ranges` bracketing each sequence's keys).
//!
//! Lights, ribbons and particle emitters carry over (see [`convert_particles`] for the
//! over-life curves 1.12 holds as three fixed keys); the texture-combiner combos, which 1.12 has
//! no slot for, are dropped. 1.12's PlayableAnimationLookup, which WotLK dropped, is rebuilt from
//! a 1.12 model's own table (see [`playable_lookup`]).

use std::collections::HashMap;

use anyhow::{bail, ensure, Context, Result};

/// Everything a v264 model is made of: the `.m2`, its `.skin` files in order, and the `.anim`
/// files by `(animation id, variation)`.
pub struct Wotlk<'a> {
    pub m2: &'a [u8],
    pub skins: Vec<Vec<u8>>,
    pub anims: HashMap<(u16, u16), Vec<u8>>,
}

/// One 1.12 PlayableAnimationLookup entry: the sequence id an animation id plays, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playable {
    pub resolved: u16,
    pub flags: u16,
}

/// Sequence flag: the keys are in the `.m2` itself, not in an `.anim` file.
const SEQ_IN_M2: u32 = 0x20;
/// Sequence flag: an alias of the sequence `alias_next` names; it carries no keys of its own.
const SEQ_ALIAS: u32 = 0x40;
/// The gap left between two sequences' windows on the shared timeline, in ms.
const GAP_MS: u32 = 16;
/// The 1.12 header's size.
const V256_HEADER: usize = 0x144;
/// A geoset 1.12 never shows: it hides every id up to 1700 that it has not picked, and picks none
/// of 1700 itself, while every id above 1700 always shows.
const HIDDEN_GEOSET: u16 = 1700;

/// The geoset a WotLK submesh keeps in 1.12: the Death Knight eye glow (1703) and the belt
/// buckles (18xx), which WotLK shows only when asked, are hidden; the rest keep theirs.
fn geoset_1_12(id: u16) -> u16 {
    match id {
        1703 | 1800..=1899 => HIDDEN_GEOSET,
        _ => id,
    }
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .with_context(|| format!("truncated at {o:#x}"))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .with_context(|| format!("truncated at {o:#x}"))
}
fn bytes(b: &[u8], o: usize, n: usize) -> Result<&[u8]> {
    b.get(o..o + n)
        .with_context(|| format!("truncated: {n} bytes at {o:#x}"))
}
/// An `M2Array`: `(count, offset)`.
fn arr(b: &[u8], o: usize) -> Result<(usize, usize)> {
    Ok((u32_at(b, o)? as usize, u32_at(b, o + 4)? as usize))
}
fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// How a track's values convert.
#[derive(Clone, Copy)]
enum Value {
    /// Copied as is, this many bytes per key.
    Same(usize),
    /// WotLK's compressed quaternion (4 x i16) to 1.12's 4 x f32.
    CompressedQuat,
    /// An `M2TrackBase`: timestamps only (animation events).
    None,
}

impl Value {
    fn src_size(self) -> usize {
        match self {
            Value::Same(n) => n,
            Value::CompressedQuat => 8,
            Value::None => 0,
        }
    }
    fn convert(self, src: &[u8], out: &mut Vec<u8>) {
        match self {
            Value::Same(_) => out.extend_from_slice(src),
            Value::CompressedQuat => {
                for i in 0..4 {
                    let s = i32::from(i16::from_le_bytes([src[i * 2], src[i * 2 + 1]]));
                    let f = if s < 0 { s + 32768 } else { s - 32767 } as f32 / 32767.0;
                    out.extend_from_slice(&f.to_le_bytes());
                }
            }
            Value::None => {}
        }
    }
}

/// A track inside a record: source offset, destination offset, its values, and the value a
/// sequence without keys rests at.
struct Track<'d> {
    src: usize,
    dst: usize,
    value: Value,
    default: Option<&'d [u8]>,
}

/// One sequence as read from v264, with its window on the 1.12 timeline.
struct Seq {
    id: u16,
    variation: u16,
    duration: u32,
    flags: u32,
    /// The sequence whose keys and window this one uses (itself unless an alias).
    target: usize,
    start: u32,
}

struct Writer<'a> {
    src: &'a Wotlk<'a>,
    seqs: Vec<Seq>,
    out: Vec<u8>,
}

impl<'a> Writer<'a> {
    fn align(&mut self) {
        while !self.out.len().is_multiple_of(16) {
            self.out.push(0);
        }
    }
    /// Append `data` (aligned) and return its offset.
    fn push(&mut self, data: &[u8]) -> u32 {
        self.align();
        let at = self.out.len() as u32;
        self.out.extend_from_slice(data);
        at
    }
    /// Reserve `n` zero bytes (aligned) and return their offset.
    fn reserve(&mut self, n: usize) -> usize {
        self.align();
        let at = self.out.len();
        self.out.resize(at + n, 0);
        at
    }
    fn put_u32(&mut self, at: usize, v: u32) {
        self.out[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn put_arr(&mut self, at: usize, count: usize, ofs: u32) {
        self.put_u32(at, count as u32);
        self.put_u32(at + 4, if count == 0 { 0 } else { ofs });
    }
    fn put(&mut self, at: usize, data: &[u8]) {
        self.out[at..at + data.len()].copy_from_slice(data);
    }

    /// Copy a plain array of `size`-byte records from source header slot `src_at` to
    /// destination header slot `dst_at`.
    fn copy_array(&mut self, src_at: usize, dst_at: usize, size: usize) -> Result<()> {
        let m2 = self.src.m2;
        let (n, o) = arr(m2, src_at)?;
        let data = bytes(m2, o, n * size)?.to_vec();
        let at = self.push(&data);
        self.put_arr(dst_at, n, at);
        Ok(())
    }

    /// The bytes a sequence's keys live in: the `.m2`, or its `.anim` file.
    fn key_source(&self, s: usize) -> Option<&'a [u8]> {
        let q = &self.seqs[s];
        if q.flags & SEQ_IN_M2 != 0 {
            Some(self.src.m2)
        } else {
            self.src.anims.get(&(q.id, q.variation)).map(Vec::as_slice)
        }
    }

    /// The keys of sub-array `s` of a track (timestamp arrays at `to`, value arrays at `vo`),
    /// read from `buf`, timestamps capped at `cap` and shifted by `base`.
    #[allow(clippy::too_many_arguments)]
    fn read_sub(
        &self,
        (to, vo): (usize, usize),
        s: usize,
        value: Value,
        buf: &[u8],
        (base, cap): (u32, u32),
        times: &mut Vec<u32>,
        vals: &mut Vec<u8>,
    ) -> Result<usize> {
        let m2 = self.src.m2;
        let vsize = value.src_size();
        let (cn, co) = arr(m2, to + s * 8)?;
        let (vcn, vco) = match value {
            Value::None => (cn, 0),
            _ => arr(m2, vo + s * 8)?,
        };
        ensure!(cn == vcn, "sub-array {s}: {cn} timestamps, {vcn} values");
        for k in 0..cn {
            times.push(base + u32_at(buf, co + k * 4)?.min(cap));
            value.convert(bytes(buf, vco + k * vsize, vsize)?, vals);
        }
        Ok(cn)
    }

    /// Convert the v264 track at `src_t` (20 bytes) into a v256 track (28 bytes) at `dst_t`. A
    /// sequence without keys, on a track that keys any other, gets one `default` key at its
    /// start, as 1.12 would otherwise sample a neighbouring sequence's keys.
    fn track(&mut self, src_t: usize, dst_t: usize, value: Value, default: Option<&[u8]>) -> Result<()> {
        let m2 = self.src.m2;
        let interp = u16_at(m2, src_t)?;
        let gseq = u16_at(m2, src_t + 2)?;
        let (tn, to) = arr(m2, src_t + 4)?;
        let (vn, vo) = match value {
            Value::None => (tn, 0),
            _ => arr(m2, src_t + 12)?,
        };
        ensure!(tn == vn, "track at {src_t:#x}: {tn} timestamp arrays, {vn} value arrays");

        let mut times: Vec<u32> = Vec::new();
        let mut vals: Vec<u8> = Vec::new();
        let mut ranges: Vec<(u32, u32)> = Vec::new();

        if gseq != 0xffff {
            // A global sequence's track: one sub-array, its own clock, no ranges.
            if tn > 0 {
                self.read_sub((to, vo), 0, value, m2, (0, u32::MAX), &mut times, &mut vals)?;
            }
        } else {
            let n_seq = self.seqs.len();
            let mut any = false;
            for s in 0..tn.min(n_seq) {
                if self.seqs[s].target == s && u32_at(m2, to + s * 8)? > 0 {
                    any = true;
                }
            }
            let mut own: Vec<Option<(u32, u32)>> = vec![None; n_seq];
            #[allow(clippy::needless_range_loop)] // `s` is also the sub-array index
            for s in 0..n_seq {
                if self.seqs[s].target != s {
                    continue;
                }
                let first = times.len() as u32;
                let window = (self.seqs[s].start, self.seqs[s].duration);
                let mut n = 0;
                if s < tn {
                    if let Some(buf) = self.key_source(s) {
                        n = self.read_sub((to, vo), s, value, buf, window, &mut times, &mut vals)?;
                    }
                }
                if n == 0 && any {
                    if let Some(d) = default {
                        times.push(window.0);
                        vals.extend_from_slice(d);
                        n = 1;
                    }
                }
                own[s] = (n > 0).then(|| (first, first + n as u32 - 1));
            }
            if any {
                ranges = (0..n_seq)
                    .map(|s| own[self.seqs[s].target].unwrap_or((0, 0)))
                    .collect();
            }
        }

        let rbytes: Vec<u8> = ranges
            .iter()
            .flat_map(|&(a, b)| [a.to_le_bytes(), b.to_le_bytes()].concat())
            .collect();
        let tbytes: Vec<u8> = times.iter().flat_map(|t| t.to_le_bytes()).collect();
        let r_at = self.push(&rbytes);
        let t_at = self.push(&tbytes);
        let v_at = self.push(&vals);
        self.put(dst_t, &interp.to_le_bytes());
        self.put(dst_t + 2, &gseq.to_le_bytes());
        self.put_arr(dst_t + 4, ranges.len(), r_at);
        self.put_arr(dst_t + 12, times.len(), t_at);
        if !matches!(value, Value::None) {
            self.put_arr(dst_t + 20, times.len(), v_at);
        }
        Ok(())
    }

    /// Convert an array of records with tracks from source header slot `src_at` to destination
    /// slot `dst_at`: each `(src, dst, len)` of `fixed` copied, each `(src, dst, element size)`
    /// of `arrays` copied out and re-pointed, then each track converted. Returns where the
    /// records start.
    fn records(
        &mut self,
        (src_at, src_size): (usize, usize),
        (dst_at, dst_size): (usize, usize),
        fixed: &[(usize, usize, usize)],
        arrays: &[(usize, usize, usize)],
        tracks: &[Track],
    ) -> Result<usize> {
        let m2 = self.src.m2;
        let (n, o) = arr(m2, src_at)?;
        let base = self.reserve(n * dst_size);
        self.put_arr(dst_at, n, base as u32);
        for i in 0..n {
            let (s, d) = (o + i * src_size, base + i * dst_size);
            for &(so, dofs, len) in fixed {
                let b = bytes(m2, s + so, len)?.to_vec();
                self.put(d + dofs, &b);
            }
            for &(so, dofs, size) in arrays {
                let (an, ao) = arr(m2, s + so)?;
                let data = bytes(m2, ao, an * size)?.to_vec();
                let at = self.push(&data);
                self.put_arr(d + dofs, an, at);
            }
            for t in tracks {
                self.track(s + t.src, d + t.dst, t.value, t.default)?;
            }
        }
        Ok(base)
    }
}

/// The 1.12 PlayableAnimationLookup for a model with the sequence ids `has`, from a 1.12
/// model's own table `template`: an id the model plays itself maps to itself; otherwise the
/// template's answer if the model has it; otherwise Stand.
pub fn playable_lookup(has: &[u16], template: &[Playable]) -> Vec<Playable> {
    template
        .iter()
        .enumerate()
        .map(|(id, t)| {
            if has.contains(&(id as u16)) {
                Playable { resolved: id as u16, flags: 0 }
            } else if has.contains(&t.resolved) {
                *t
            } else {
                Playable { resolved: 0, flags: 0 }
            }
        })
        .collect()
}

/// Read a 1.12 (v256) model's PlayableAnimationLookup, the template for [`playable_lookup`].
pub fn read_playable(v256: &[u8]) -> Result<Vec<Playable>> {
    ensure!(v256.get(0..4) == Some(b"MD20"), "not an M2");
    let (n, o) = arr(v256, 0x2c)?;
    (0..n)
        .map(|i| {
            Ok(Playable {
                resolved: u16_at(v256, o + i * 4)?,
                flags: u16_at(v256, o + i * 4 + 2)?,
            })
        })
        .collect()
}

/// The `(id, variation)` of every sequence whose keys live in an `.anim` file.
pub fn anim_files(m2: &[u8]) -> Result<Vec<(u16, u16)>> {
    let (n, o) = arr(m2, 0x1c)?;
    let mut out = Vec::new();
    for i in 0..n {
        let r = o + i * 64;
        if u32_at(m2, r + 12)? & (SEQ_IN_M2 | SEQ_ALIAS) == 0 {
            out.push((u16_at(m2, r)?, u16_at(m2, r + 2)?));
        }
    }
    Ok(out)
}

/// How many `.skin` files the model has.
pub fn skin_count(m2: &[u8]) -> Result<usize> {
    Ok(u32_at(m2, 0x44)? as usize)
}

/// Read the v264 sequences and lay their windows out on one timeline; an alias takes its
/// target's window.
fn sequences(m2: &[u8]) -> Result<Vec<Seq>> {
    let (sn, so) = arr(m2, 0x1c)?;
    let mut seqs = Vec::with_capacity(sn);
    for i in 0..sn {
        let r = so + i * 64;
        seqs.push(Seq {
            id: u16_at(m2, r)?,
            variation: u16_at(m2, r + 2)?,
            duration: u32_at(m2, r + 4)?,
            flags: u32_at(m2, r + 12)?,
            target: i,
            start: 0,
        });
    }
    for i in 0..sn {
        let mut t = i;
        for _ in 0..sn {
            if seqs[t].flags & SEQ_ALIAS == 0 {
                break;
            }
            t = usize::from(u16_at(m2, so + t * 64 + 62)?);
            if t >= sn {
                bail!("sequence {i}: alias past the end");
            }
        }
        seqs[i].target = t;
    }
    let mut cursor = 0u32;
    for s in seqs.iter_mut().filter(|s| s.flags & SEQ_ALIAS == 0) {
        s.start = cursor;
        cursor += s.duration + GAP_MS;
    }
    for i in 0..sn {
        let t = seqs[i].target;
        (seqs[i].start, seqs[i].duration) = (seqs[t].start, seqs[t].duration);
    }
    Ok(seqs)
}

/// Rewrite a v264 model as v256. `template` is a 1.12 model's PlayableAnimationLookup.
pub fn convert(src: &Wotlk, template: &[Playable]) -> Result<Vec<u8>> {
    let m2 = src.m2;
    ensure!(m2.get(0..4) == Some(b"MD20"), "not an MD20 model");
    let version = u32_at(m2, 4)?;
    ensure!(version == 264, "M2 version {version}, expected 264");
    ensure!(!src.skins.is_empty(), "no skin profiles");

    let seqs = sequences(m2)?;
    let (sn, so) = arr(m2, 0x1c)?;
    let mut w = Writer { src, seqs, out: Vec::new() };
    w.reserve(V256_HEADER);
    w.put(0, b"MD20");
    w.put_u32(4, 256);
    w.copy_array(0x08, 0x08, 1)?; // name, NUL included
    // Flags without 0x08 (texture-combiner combos, which 1.12 has no slot for).
    w.put_u32(0x10, u32_at(m2, 0x10)? & !0x08);
    w.copy_array(0x14, 0x14, 4)?; // global sequences

    // Sequences, 64 to 68 bytes: a window instead of a duration.
    let base = w.reserve(sn * 68);
    w.put_arr(0x1c, sn, base as u32);
    for i in 0..sn {
        let (r, d) = (so + i * 64, base + i * 68);
        let s = &w.seqs[i];
        let (start, end) = (s.start, s.start + s.duration);
        let flags = (s.flags & !SEQ_ALIAS) | SEQ_IN_M2;
        let head = bytes(m2, r, 4)?.to_vec(); // id, variation
        let speed = bytes(m2, r + 8, 4)?.to_vec();
        let tail = bytes(m2, r + 16, 48)?.to_vec(); // frequency .. alias_next
        w.put(d, &head);
        w.put_u32(d + 4, start);
        w.put_u32(d + 8, end);
        w.put(d + 12, &speed);
        w.put_u32(d + 16, flags);
        w.put(d + 20, &tail);
    }
    w.copy_array(0x24, 0x24, 2)?; // sequence lookup

    let has: Vec<u16> = w.seqs.iter().filter(|s| s.duration > 0).map(|s| s.id).collect();
    let playable: Vec<u8> = playable_lookup(&has, template)
        .iter()
        .flat_map(|p| [p.resolved.to_le_bytes(), p.flags.to_le_bytes()].concat())
        .collect();
    let at = w.push(&playable);
    w.put_arr(0x2c, playable.len() / 4, at);

    let zero3 = floats(&[0., 0., 0.]);
    let one3 = floats(&[1., 1., 1.]);
    let ident_rot = floats(&[0., 0., 0., 1.]);
    let opaque = 0x7fffu16.to_le_bytes();
    let on = [1u8];

    // Bones, 88 to 108 bytes: no name CRC, 28-byte tracks.
    w.records(
        (0x2c, 88),
        (0x34, 108),
        &[(0, 0, 12), (76, 96, 12)],
        &[],
        &[
            Track { src: 16, dst: 12, value: Value::Same(12), default: Some(&zero3) },
            Track { src: 36, dst: 40, value: Value::CompressedQuat, default: Some(&ident_rot) },
            Track { src: 56, dst: 68, value: Value::Same(12), default: Some(&one3) },
        ],
    )?;
    w.copy_array(0x34, 0x3c, 2)?; // key bone lookup
    w.copy_array(0x3c, 0x44, 48)?; // vertices

    // Skin profiles, embedded: the arrays re-pointed, submeshes cut from 48 to 32 bytes.
    let views = w.reserve(src.skins.len() * 44);
    w.put_arr(0x4c, src.skins.len(), views as u32);
    for (i, skin) in src.skins.iter().enumerate() {
        ensure!(skin.get(0..4) == Some(b"SKIN"), "skin {i}: no SKIN magic");
        let v = views + i * 44;
        for (k, size) in [(0usize, 2usize), (1, 2), (2, 4)] {
            let (n, o) = arr(skin, 4 + k * 8)?;
            let data = bytes(skin, o, n * size)?.to_vec();
            let at = w.push(&data);
            w.put_arr(v + k * 8, n, at);
        }
        let (n, o) = arr(skin, 28)?;
        let mut subs = Vec::with_capacity(n * 32);
        for j in 0..n {
            let s = bytes(skin, o + j * 48, 48)?;
            ensure!(u16_at(s, 2)? == 0, "skin {i} submesh {j} starts past index 65535");
            subs.extend_from_slice(&geoset_1_12(u16_at(s, 0)?).to_le_bytes());
            subs.extend_from_slice(&s[2..32]);
        }
        let at = w.push(&subs);
        w.put_arr(v + 24, n, at);
        let (n, o) = arr(skin, 36)?;
        let data = bytes(skin, o, n * 24)?.to_vec();
        let at = w.push(&data);
        w.put_arr(v + 32, n, at);
        let bone_max = u32_at(skin, 44)?;
        w.put_u32(v + 40, bone_max);
    }

    // Colours, 40 to 56.
    w.records(
        (0x48, 40),
        (0x54, 56),
        &[],
        &[],
        &[
            Track { src: 0, dst: 0, value: Value::Same(12), default: Some(&one3) },
            Track { src: 20, dst: 28, value: Value::Same(2), default: Some(&opaque) },
        ],
    )?;

    // Textures: 16-byte records whose filename is an array of its own.
    let (tn, to) = arr(m2, 0x50)?;
    let tex = w.reserve(tn * 16);
    w.put_arr(0x5c, tn, tex as u32);
    for i in 0..tn {
        let r = to + i * 16;
        let head = bytes(m2, r, 8)?.to_vec();
        w.put(tex + i * 16, &head);
        let (n, o) = arr(m2, r + 8)?;
        let name = bytes(m2, o, n)?.to_vec();
        let at = w.push(&name);
        w.put_arr(tex + i * 16 + 8, n, at);
    }

    // Texture weights, 20 to 28; 0x6c texture flipbooks: none; texture transforms, 60 to 84.
    w.records(
        (0x58, 20),
        (0x64, 28),
        &[],
        &[],
        &[Track { src: 0, dst: 0, value: Value::Same(2), default: Some(&opaque) }],
    )?;
    w.records(
        (0x60, 60),
        (0x74, 84),
        &[],
        &[],
        &[
            Track { src: 0, dst: 0, value: Value::Same(12), default: Some(&zero3) },
            Track { src: 20, dst: 28, value: Value::Same(16), default: Some(&ident_rot) },
            Track { src: 40, dst: 56, value: Value::Same(12), default: Some(&one3) },
        ],
    )?;
    w.copy_array(0x68, 0x7c, 2)?; // replaceable texture lookup
    w.copy_array(0x70, 0x84, 4)?; // render flags
    w.copy_array(0x78, 0x8c, 2)?; // bone lookup
    w.copy_array(0x80, 0x94, 2)?; // texture lookup
    w.copy_array(0x88, 0x9c, 2)?; // texture unit lookup
    w.copy_array(0x90, 0xa4, 2)?; // transparency lookup
    w.copy_array(0x98, 0xac, 2)?; // texture transform lookup
    let bounds = bytes(m2, 0xa0, 56)?.to_vec(); // bounding box + radius, collision box + radius
    w.put(0xb4, &bounds);
    w.copy_array(0xd8, 0xec, 2)?; // collision triangles
    w.copy_array(0xe0, 0xf4, 12)?; // collision vertices
    w.copy_array(0xe8, 0xfc, 12)?; // collision normals

    // Attachments, 40 to 48; events, 36 to 44.
    w.records(
        (0xf0, 40),
        (0x104, 48),
        &[(0, 0, 20)],
        &[],
        &[Track { src: 20, dst: 20, value: Value::Same(1), default: Some(&on) }],
    )?;
    w.copy_array(0xf8, 0x10c, 2)?; // attachment lookup
    w.records(
        (0x100, 36),
        (0x114, 44),
        &[(0, 0, 24)],
        &[],
        &[Track { src: 24, dst: 24, value: Value::None, default: None }],
    )?;
    // Lights, 156 to 212: type and bone, position, then seven tracks.
    w.records(
        (0x108, 0x9c),
        (0x11c, 0xd4),
        &[(0, 0, 0x10)],
        &[],
        &[
            Track { src: 0x10, dst: 0x10, value: Value::Same(12), default: None }, // ambient colour
            Track { src: 0x24, dst: 0x2c, value: Value::Same(4), default: None },  // ambient intensity
            Track { src: 0x38, dst: 0x48, value: Value::Same(12), default: None }, // diffuse colour
            Track { src: 0x4c, dst: 0x64, value: Value::Same(4), default: None },  // diffuse intensity
            Track { src: 0x60, dst: 0x80, value: Value::Same(4), default: None },  // attenuation start
            Track { src: 0x74, dst: 0x9c, value: Value::Same(4), default: None },  // attenuation end
            Track { src: 0x88, dst: 0xb8, value: Value::Same(1), default: Some(&on) }, // visibility
        ],
    )?;
    // Cameras, 100 to 124 (spline keys are the same in both).
    w.records(
        (0x110, 100),
        (0x124, 124),
        &[(0, 0, 16), (36, 44, 12), (68, 84, 12)],
        &[],
        &[
            Track { src: 16, dst: 16, value: Value::Same(36), default: None },
            Track { src: 48, dst: 56, value: Value::Same(36), default: None },
            Track { src: 80, dst: 96, value: Value::Same(12), default: None },
        ],
    )?;
    w.copy_array(0x118, 0x12c, 2)?; // camera lookup
    // Ribbons, 176 to 220: WotLK's priority plane (the last 4 bytes) has no 1.12 slot.
    w.records(
        (0x120, 0xb0),
        (0x134, 0xdc),
        &[(0, 0, 0x14), (0x74, 0x94, 0x10)], // id, bone, position; edge rate .. tile counts
        &[(0x14, 0x14, 2), (0x1c, 0x1c, 2)], // textures, materials
        &[
            Track { src: 0x24, dst: 0x24, value: Value::Same(12), default: Some(&one3) }, // colour
            Track { src: 0x38, dst: 0x40, value: Value::Same(2), default: Some(&opaque) }, // alpha
            Track { src: 0x4c, dst: 0x5c, value: Value::Same(4), default: None }, // height above
            Track { src: 0x60, dst: 0x78, value: Value::Same(4), default: None }, // height below
            Track { src: 0x84, dst: 0xa4, value: Value::Same(2), default: None }, // texture slot
            Track { src: 0x98, dst: 0xc0, value: Value::Same(1), default: Some(&on) }, // visibility
        ],
    )?;
    convert_particles(&mut w, &on)?;
    Ok(w.out)
}
/// The v264 particle emitter's size.
const V264_PARTICLE: usize = 0x1dc;
/// The v256 particle emitter's size.
const V256_PARTICLE: usize = 0x1f8;

/// One key of an over-life curve: its time as a fraction of the particle's life, and up to
/// three components.
type LifeKey = (f32, [f32; 3]);

/// A WotLK `M2PartTrack` at `at` (timestamps as fixed16 fractions of the particle's life, 32768
/// the whole of it, then `size`-byte values `decode` reads).
fn life_keys(m2: &[u8], at: usize, size: usize, decode: impl Fn(&[u8]) -> [f32; 3]) -> Result<Vec<LifeKey>> {
    let (tn, to) = arr(m2, at)?;
    let (vn, vo) = arr(m2, at + 8)?;
    (0..tn.min(vn))
        .map(|k| {
            let t = f32::from(u16_at(m2, to + k * 2)?) / 32768.0;
            Ok((t, decode(bytes(m2, vo + k * size, size)?)))
        })
        .collect()
}

/// An over-life curve's value just after life fraction `t`: where two keys share the time `t`
/// (a jump), the later one's value; otherwise [`life_at`].
fn life_after(keys: &[LifeKey], t: f32, default: [f32; 3]) -> [f32; 3] {
    keys.iter().rev().find(|k| k.0 == t).map_or_else(|| life_at(keys, t, default), |k| k.1)
}

/// Whether 1.12's start, midpoint and end keys express a curve exactly: up to three keys, or
/// four whose middle two share a time (a jump at the midpoint, as WotLK writes 1.12's two cell
/// ramps).
fn life_fits(keys: &[LifeKey]) -> bool {
    keys.len() <= 3 || (keys.len() == 4 && keys[1].0 == keys[2].0)
}

/// The midpoint a curve authors: its middle key, or the time of its jump.
fn life_mid(keys: &[LifeKey]) -> Option<f32> {
    (keys.len() == 3 || (keys.len() == 4 && life_fits(keys))).then(|| keys[1].0)
}

/// An over-life curve's value at life fraction `t`, linear between keys (at a jump, the earlier
/// key's value); `default` without keys.
fn life_at(keys: &[LifeKey], t: f32, default: [f32; 3]) -> [f32; 3] {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return default;
    };
    if t <= first.0 {
        return first.1;
    }
    if t >= last.0 {
        return last.1;
    }
    let i = keys.iter().position(|k| k.0 >= t).unwrap_or(keys.len() - 1);
    let ((t0, a), (t1, b)) = (keys[i - 1], keys[i]);
    let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * f)
}

/// The over-life curves of one WotLK emitter: colour (RGB, 0..255), alpha (0..1), size (x, y),
/// head cell and tail cell.
struct LifeCurves {
    colour: Vec<LifeKey>,
    alpha: Vec<LifeKey>,
    scale: Vec<LifeKey>,
    head: Vec<LifeKey>,
    tail: Vec<LifeKey>,
}

impl LifeCurves {
    /// Read the five `M2PartTrack`s of the v264 emitter at `e`.
    fn read(m2: &[u8], e: usize) -> Result<Self> {
        let f = |b: &[u8], i: usize| f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let fixed16 = |b: &[u8]| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32767.0;
        let cell = |b: &[u8]| [f32::from(u16::from_le_bytes([b[0], b[1]])), 0.0, 0.0];
        Ok(LifeCurves {
            colour: life_keys(m2, e + 0x104, 12, |b| [f(b, 0), f(b, 4), f(b, 8)])?,
            alpha: life_keys(m2, e + 0x114, 2, |b| [fixed16(b), 0.0, 0.0])?,
            scale: life_keys(m2, e + 0x124, 8, |b| [f(b, 0), f(b, 4), 0.0])?,
            head: life_keys(m2, e + 0x13c, 2, cell)?,
            tail: life_keys(m2, e + 0x14c, 2, cell)?,
        })
    }

    fn all(&self) -> [&[LifeKey]; 5] {
        [&self.colour, &self.alpha, &self.scale, &self.head, &self.tail]
    }

    /// 1.12's over-life block (emitter +0x14c..+0x17c): a midpoint, three colours (packed
    /// BGRA, alpha in the top byte), three sizes and the head and tail cell ramps either side of
    /// the midpoint. WotLK authors its curves as 1.12's keys (start, middle, end; a cell ramp
    /// as four keys jumping at the middle); the midpoint is the first such curve's (else half
    /// the life), and a curve that does not fit ([`life_fits`]) is sampled at those points.
    fn v256(&self) -> [u8; 0x30] {
        let mid = self.all().into_iter().find_map(life_mid).unwrap_or(0.5).clamp(0.01, 0.99);
        let mut out = [0u8; 0x30];
        out[0..4].copy_from_slice(&mid.to_le_bytes());
        let byte = |v: f32| v.round().clamp(0.0, 255.0) as u32;
        for (k, t) in [0.0, mid, 1.0].into_iter().enumerate() {
            let rgb = life_at(&self.colour, t, [255.0; 3]);
            let a = life_at(&self.alpha, t, [1.0; 3])[0];
            let packed = (byte(a * 255.0) << 24) | (byte(rgb[0]) << 16) | (byte(rgb[1]) << 8) | byte(rgb[2]);
            out[4 + k * 4..8 + k * 4].copy_from_slice(&packed.to_le_bytes());
            let size = life_at(&self.scale, t, [1.0; 3])[0];
            out[0x10 + k * 4..0x14 + k * 4].copy_from_slice(&size.to_le_bytes());
        }
        let cell = |v: [f32; 3]| v[0].round().clamp(0.0, 65535.0) as u16;
        let ramps = |keys: &[LifeKey]| {
            let at = |t| cell(life_at(keys, t, [0.0; 3]));
            [at(0.0), at(mid), cell(life_after(keys, mid, [0.0; 3])), at(1.0)]
        };
        let ([h0, h1, h2, h3], [t0, t1, t2, t3]) = (ramps(&self.head), ramps(&self.tail));
        // Head: A from, A to, A repeat, B from, B to, B repeat; tail: A from, A to, B from, B to.
        let words = [h0, h1, 1, h2, h3, 1, t0, t1, t2, t3];
        for (i, w) in words.iter().enumerate() {
            out[0x1c + i * 2..0x1e + i * 2].copy_from_slice(&w.to_le_bytes());
        }
        out
    }
}

/// Particle emitters, 476 to 504 bytes. The ten emission tracks and the on/off track convert
/// like any track, the over-life curves become 1.12's fixed keys ([`LifeCurves::v256`]), the
/// scalars after them move up. What 1.12 has no slot for is left out: the ParticleColor.dbc
/// tint index (creatures' tinted spell effects), the lifespan, rate and size variations, and
/// the random starting spin (the spin rate carries).
fn convert_particles(w: &mut Writer, on: &[u8]) -> Result<()> {
    let m2 = w.src.m2;
    let f32_track = |src: usize, dst: usize| Track { src, dst, value: Value::Same(4), default: None };
    let base = w.records(
        (0x128, V264_PARTICLE),
        (0x13c, V256_PARTICLE),
        &[
            (0, 0, 0x18),       // id, flags, position, bone, texture
            (0x2c, 0x2c, 8),    // particle type, head or tail, tile rotation, rows, columns
            (0x15c, 0x17c, 4),  // tail length
            (0x160, 0x180, 24), // twinkle speed, percent, scale range; burst multiplier; drag
            (0x180, 0x198, 4),  // spin
            (0x188, 0x19c, 56), // tumble box, wind vector and time, follow speeds and scales
        ],
        &[(0x18, 0x18, 1), (0x20, 0x20, 1), (0x1c0, 0x1d4, 12)], // geometry, recursion, spline
        &[
            f32_track(0x34, 0x34),  // speed
            f32_track(0x48, 0x50),  // speed variation
            f32_track(0x5c, 0x6c),  // vertical range
            f32_track(0x70, 0x88),  // horizontal range
            f32_track(0x84, 0xa4),  // gravity
            f32_track(0x98, 0xc0),  // lifespan
            f32_track(0xb0, 0xdc),  // emission rate
            f32_track(0xc8, 0xf8),  // area length
            f32_track(0xdc, 0x114), // area width
            f32_track(0xf0, 0x130), // z source
            Track { src: 0x1c8, dst: 0x1dc, value: Value::Same(1), default: Some(on) }, // enabled
        ],
    )?;
    let (n, o) = arr(m2, 0x128)?;
    for i in 0..n {
        let (e, d) = (o + i * V264_PARTICLE, base + i * V256_PARTICLE);
        // The flags 1.12 knows (WotLK's from 0x10000 up mean nothing to it); blend and emitter
        // type, a byte each in WotLK, a u16 each in 1.12.
        w.put_u32(d + 4, u32_at(m2, e + 4)? & 0xffff);
        let head = bytes(m2, e + 0x28, 2)?.to_vec();
        w.put(d + 0x28, &u16::from(head[0]).to_le_bytes());
        w.put(d + 0x2a, &u16::from(head[1]).to_le_bytes());
        let block = LifeCurves::read(m2, e)?.v256();
        w.put(d + 0x14c, &block);
    }
    Ok(())
}

/// The models a v256 model's particle emitters spawn (geometry and recursion models), as named.
pub fn emitter_models(v256: &[u8]) -> Result<Vec<String>> {
    let (n, o) = arr(v256, 0x13c)?;
    let mut out = Vec::new();
    for i in 0..n {
        for slot in [0x18, 0x20] {
            let (len, at) = arr(v256, o + i * V256_PARTICLE + slot)?;
            let raw = bytes(v256, at, len)?;
            let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
            let name = String::from_utf8_lossy(&raw[..end]).into_owned();
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    Ok(out)
}

/// What [`convert`] carries only in part, counted over a v264 model's emitters: those tinted
/// from ParticleColor.dbc, and those with an over-life curve of more than three keys (resampled
/// to three).
pub fn approximated(m2: &[u8]) -> Result<(usize, usize)> {
    let (n, o) = arr(m2, 0x128)?;
    let (mut tinted, mut resampled) = (0, 0);
    for i in 0..n {
        let e = o + i * V264_PARTICLE;
        if u16_at(m2, e + 0x2a)? != 0 {
            tinted += 1;
        }
        if !LifeCurves::read(m2, e)?.all().into_iter().all(life_fits) {
            resampled += 1;
        }
    }
    Ok((tinted, resampled))
}

/// Read a v256 model's particle emitters, ribbons and lights back with the client's own readers:
/// every record the header counts must come back, each emitter with a texture or a model to
/// draw and sane over-life keys.
pub fn check_effects(v256: &[u8]) -> Result<()> {
    let count = |at: usize| -> Result<usize> { Ok(u32_at(v256, at)? as usize) };
    let emitters = benilla_formats::parse_m2_particle_emitters(v256)?;
    ensure!(emitters.len() == count(0x13c)?, "{} of {} particle emitters read back", emitters.len(), count(0x13c)?);
    for (i, e) in emitters.iter().enumerate() {
        ensure!(
            e.texture.is_some() || e.geometry_model.is_some(),
            "particle emitter {i}: no texture and no model"
        );
        let life = &e.over_life;
        ensure!(life.mid > 0.0 && life.mid < 1.0, "particle emitter {i}: midpoint {}", life.mid);
        ensure!(life.scale.iter().all(|s| s.is_finite()), "particle emitter {i}: sizes {:?}", life.scale);
        ensure!(e.params.peak_lifespan().is_finite(), "particle emitter {i}: lifespan");
    }
    let ribbons = benilla_formats::parse_m2_ribbon_emitters(v256)?;
    ensure!(ribbons.len() == count(0x134)?, "{} of {} ribbons read back", ribbons.len(), count(0x134)?);
    let lights = benilla_formats::parse_m2_lights(v256);
    ensure!(lights.len() == count(0x11c)?, "{} of {} lights read back", lights.len(), count(0x11c)?);
    Ok(())
}

/// The file names of a v256 model's hard-coded (type 0) textures.
pub fn hardcoded_textures(v256: &[u8]) -> Result<Vec<String>> {
    let (n, o) = arr(v256, 0x5c)?;
    let mut out = Vec::new();
    for i in 0..n {
        let r = o + i * 16;
        if u32_at(v256, r)? != 0 {
            continue;
        }
        let (len, at) = arr(v256, r + 8)?;
        let raw = bytes(v256, at, len)?;
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        let name = String::from_utf8_lossy(&raw[..end]).into_owned();
        if !name.is_empty() {
            out.push(name);
        }
    }
    Ok(out)
}

/// A v256 model's animation events: the offset of each one's payload, its tag and its payload.
pub fn events(v256: &[u8]) -> Result<Vec<(usize, [u8; 4], u32)>> {
    let (n, o) = arr(v256, 0x114)?;
    (0..n)
        .map(|i| {
            let r = o + i * 44;
            let tag: [u8; 4] = bytes(v256, r, 4)?.try_into()?;
            Ok((r + 4, tag, u32_at(v256, r + 4)?))
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn quat(words: [i16; 4]) -> [f32; 4] {
        let src: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut out = Vec::new();
        Value::CompressedQuat.convert(&src, &mut out);
        std::array::from_fn(|i| f32::from_le_bytes(out[i * 4..i * 4 + 4].try_into().unwrap()))
    }

    #[test]
    fn compressed_quaternions_expand_to_the_unit_range() {
        // WotLK packs the identity as (32767, 32767, 32767, -1): 0, 0, 0 and 1.
        assert_eq!(quat([32767, 32767, 32767, -1]), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(quat([i16::MIN, -1, 0, 1]), [0.0, 1.0, -1.0, -32766.0 / 32767.0]);
    }

    #[test]
    fn playable_lookup_prefers_the_model_own_clip_then_the_template_then_stand() {
        let template = [
            Playable { resolved: 0, flags: 0 },
            Playable { resolved: 0, flags: 3 },
            Playable { resolved: 1, flags: 0 },
            Playable { resolved: 5, flags: 1 },
        ];
        let got = playable_lookup(&[0, 1], &template);
        assert_eq!(got[1], Playable { resolved: 1, flags: 0 }, "the model's own clip");
        assert_eq!(got[2], Playable { resolved: 1, flags: 0 }, "the template's answer, which the model has");
        assert_eq!(got[3], Playable { resolved: 0, flags: 0 }, "the template's answer is missing: Stand");
    }

    fn key(t: f32, v: f32) -> LifeKey {
        (t, [v, 0.0, 0.0])
    }

    #[test]
    fn a_doubled_middle_key_splits_the_cell_ramps_there() {
        // WotLK writes 1.12's head ramps 0..7 then 8..16 as four keys jumping at the middle.
        let head = [key(0.0, 0.0), key(0.5, 7.0), key(0.5, 8.0), key(1.0, 16.0)];
        assert!(life_fits(&head));
        let c = LifeCurves { colour: vec![], alpha: vec![], scale: vec![], head: head.to_vec(), tail: vec![] };
        let b = c.v256();
        let word = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        assert_eq!(f32::from_le_bytes(b[0..4].try_into().unwrap()), 0.5);
        assert_eq!([word(0x1c), word(0x1e), word(0x20)], [0, 7, 1], "head A and its repeat");
        assert_eq!([word(0x22), word(0x24), word(0x26)], [8, 16, 1], "head B and its repeat");
        assert_eq!([word(0x28), word(0x2a), word(0x2c), word(0x2e)], [0; 4], "no tail cells");
    }

    #[test]
    fn colour_alpha_and_size_curves_become_three_packed_keys() {
        let c = LifeCurves {
            colour: vec![(0.0, [255.0, 72.0, 0.0]), (0.25, [0.0, 0.0, 0.0]), (1.0, [255.0, 247.0, 221.0])],
            alpha: vec![key(0.0, 1.0), key(1.0, 0.0)],
            scale: vec![key(0.0, 2.0)],
            head: vec![],
            tail: vec![],
        };
        let b = c.v256();
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        assert_eq!(f(0), 0.25, "the three-key colour curve sets the midpoint");
        assert_eq!(u(4), 0xffff_4800, "opaque orange: ARGB from the top byte down");
        assert_eq!(u(8), 0xbf00_0000, "black at the midpoint, alpha 0.75 by then");
        assert_eq!(u(12), 0x00ff_f7dd);
        assert_eq!([f(0x10), f(0x14), f(0x18)], [2.0; 3], "one size key holds all life");
    }

    #[test]
    fn a_curve_with_inner_keys_does_not_fit_and_is_sampled() {
        let five = [key(0.0, 0.0), key(0.2, 1.0), key(0.5, 2.0), key(0.8, 3.0), key(1.0, 4.0)];
        assert!(!life_fits(&five));
        assert_eq!(life_at(&five, 0.35, [9.0; 3])[0], 1.5);
        assert_eq!(life_at(&[], 0.35, [9.0; 3])[0], 9.0);
    }

    #[test]
    fn only_the_glow_and_belt_geosets_are_hidden() {
        assert_eq!(geoset_1_12(1703), HIDDEN_GEOSET);
        assert_eq!(geoset_1_12(1802), HIDDEN_GEOSET);
        for kept in [0, 1, 401, 702, 1501, 1702] {
            assert_eq!(geoset_1_12(kept), kept);
        }
    }
}