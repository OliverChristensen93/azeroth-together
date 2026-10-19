//! The height map as bytes, for the bake's cache: a header, the terrain heights, the blocked and
//! water bits and the span lists of the columns that have them, all little-endian. Reading checks
//! everything it can before it allocates, so a cut-short or foreign file is refused, never
//! trusted.

use super::{Grid, HeightMap, Span};
use crate::V2;

const MAGIC: &[u8; 4] = b"NFHM";
/// Bumped whenever the layout changes.
const VERSION: u32 = 2;
/// No map comes near this many columns (Duskwood's bounds are 17 million); a header past it is
/// not ours.
const MAX_COLUMNS: u64 = 200_000_000;

impl HeightMap {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            32 + self.ground.len() * 2
                + self.blocked.len() * 8
                + self.water.len() * 8
                + self.cols.len() * 8
                + self.spans.len() * 8,
        );
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        for v in [self.grid.origin.x, self.grid.origin.y, self.grid.cell] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.grid.nx.to_le_bytes());
        out.extend_from_slice(&self.grid.ny.to_le_bytes());
        for g in &self.ground {
            out.extend_from_slice(&g.to_le_bytes());
        }
        for w in &self.blocked {
            out.extend_from_slice(&w.to_le_bytes());
        }
        for w in &self.water {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(&(self.cols.len() as u32).to_le_bytes());
        for c in &self.cols {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for s in &self.starts {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.extend_from_slice(&(self.spans.len() as u32).to_le_bytes());
        for s in &self.spans {
            out.extend_from_slice(&s.lo.to_le_bytes());
            out.extend_from_slice(&s.hi.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<HeightMap, String> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err("not a Nightfall height map".into());
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(format!("height map version {version}, not {VERSION}"));
        }
        let origin = V2::new(r.f32()?, r.f32()?);
        let cell = r.f32()?;
        let (nx, ny) = (r.u32()?, r.u32()?);
        if !origin.is_finite() || !(cell > 0.0 && cell.is_finite()) || nx == 0 || ny == 0 {
            return Err("the height map's header is broken".into());
        }
        let n = u64::from(nx) * u64::from(ny);
        if n > MAX_COLUMNS {
            return Err(format!("a height map of {n} columns is not ours"));
        }
        let n = n as usize;
        let ground = r.vec(n, 2, |b| i16::from_le_bytes([b[0], b[1]]))?;
        let blocked = r.vec(n.div_ceil(64), 8, |b| {
            u64::from_le_bytes(b.try_into().expect("eight bytes"))
        })?;
        let water = r.vec(n.div_ceil(64), 8, |b| {
            u64::from_le_bytes(b.try_into().expect("eight bytes"))
        })?;
        let count = r.u32()? as usize;
        if count > n {
            return Err("more columns with spans than columns".into());
        }
        let cols = r.vec(count, 4, le_u32)?;
        if cols.windows(2).any(|w| w[0] >= w[1]) || cols.last().is_some_and(|&c| c as usize >= n) {
            return Err("the height map's columns are out of order".into());
        }
        let starts = r.vec(count + 1, 4, le_u32)?;
        let span_count = r.u32()? as usize;
        if starts[0] != 0
            || starts[count] as usize != span_count
            || starts.windows(2).any(|w| w[0] >= w[1])
        {
            return Err("the height map's span lists are out of order".into());
        }
        let spans = r.vec(span_count, 8, |b| Span {
            lo: f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            hi: f32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        })?;
        if spans
            .iter()
            .any(|s| !s.lo.is_finite() || !s.hi.is_finite() || s.lo > s.hi)
        {
            return Err("a span in the height map is not a number".into());
        }
        if r.at != bytes.len() {
            return Err("bytes left over after the height map".into());
        }
        Ok(HeightMap {
            grid: Grid {
                origin,
                cell,
                nx,
                ny,
            },
            ground,
            cols,
            starts,
            spans,
            blocked,
            water,
        })
    }
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Bytes read in order; every read checks there are enough first.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or("the height map is cut short")?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.take(4).map(le_u32)
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.u32().map(f32::from_bits)
    }

    /// `count` items of `size` bytes each; the bytes are checked to be there before the vector
    /// is made, so a lying count costs nothing.
    fn vec<T>(
        &mut self,
        count: usize,
        size: usize,
        read: impl Fn(&[u8]) -> T,
    ) -> Result<Vec<T>, String> {
        let len = count
            .checked_mul(size)
            .ok_or("the height map is cut short")?;
        Ok(self.take(len)?.chunks_exact(size).map(read).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::super::testmap::{flat, house};
    use super::super::HeightMap;
    use crate::V2;

    fn town() -> HeightMap {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            Some(25.0),
            true,
        );
        b.block_box(V2::new(0.0, 0.0), V2::new(3.0, 3.0));
        b.set_ground(7, f32::NAN);
        b.finish()
    }

    #[test]
    fn a_map_reads_back_as_it_was_written() {
        let m = town();
        assert_eq!(HeightMap::from_bytes(&m.to_bytes()).unwrap(), m);
    }

    #[test]
    fn a_cut_short_or_foreign_file_is_refused() {
        let bytes = town().to_bytes();
        assert!(HeightMap::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(HeightMap::from_bytes(&[]).is_err());
        let mut foreign = bytes.clone();
        foreign[0] = b'X';
        assert!(HeightMap::from_bytes(&foreign).is_err());
        let mut newer = bytes.clone();
        newer[4] = 99;
        assert!(HeightMap::from_bytes(&newer).is_err());
        let mut longer = bytes;
        longer.push(0);
        assert!(HeightMap::from_bytes(&longer).is_err());
    }

    #[test]
    fn a_header_claiming_billions_of_columns_is_refused_before_allocating() {
        let mut bytes = town().to_bytes();
        // `nx` follows the magic, the version, the origin and the cell: bytes 20..24.
        bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(HeightMap::from_bytes(&bytes).is_err());
    }

    #[test]
    fn a_span_that_is_not_a_number_is_refused() {
        let m = town();
        let mut bytes = m.to_bytes();
        // The last span's `hi` is the file's last four bytes.
        let n = bytes.len();
        bytes[n - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(HeightMap::from_bytes(&bytes).is_err());
    }

    #[test]
    fn water_comes_back_from_bytes_and_a_version_1_file_is_refused() {
        let mut b = flat(50.0);
        super::super::testmap::pond(&mut b, V2::new(10.0, 10.0), V2::new(14.0, 12.0));
        let m = b.finish();
        let back = HeightMap::from_bytes(&m.to_bytes()).unwrap();
        assert!(back.is_water(V2::new(12.0, 11.0)));
        assert_eq!(back.water_columns(), m.water_columns());
        let mut old = m.to_bytes();
        old[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            HeightMap::from_bytes(&old).unwrap_err(),
            "height map version 1, not 2"
        );
    }
}
