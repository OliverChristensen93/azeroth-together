//! WDBC tables, both versions: a header, fixed-size records, a string block. Rows are kept as raw
//! record bytes so a 1.12 table can be extended without knowing its column types; a ported row is
//! built field by field and its strings appended to the block.

use anyhow::{ensure, Context, Result};

pub struct Dbc {
    pub fields: usize,
    pub record_size: usize,
    pub rows: Vec<Vec<u8>>,
    pub strings: Vec<u8>,
}

/// One field of a row being built.
#[derive(Clone, Debug)]
pub enum F {
    U(u32),
    S(String),
}

impl Dbc {
    pub fn parse(b: &[u8]) -> Result<Self> {
        ensure!(b.get(0..4) == Some(b"WDBC"), "not a WDBC table");
        let h = |i: usize| -> Result<usize> {
            Ok(u32::from_le_bytes(b.get(4 + i * 4..8 + i * 4).context("short header")?.try_into()?) as usize)
        };
        let (n, fields, record_size, string_size) = (h(0)?, h(1)?, h(2)?, h(3)?);
        let body = b.get(20..20 + n * record_size + string_size).context("truncated table")?;
        let rows = body[..n * record_size].chunks(record_size).map(<[u8]>::to_vec).collect();
        Ok(Self { fields, record_size, rows, strings: body[n * record_size..].to_vec() })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = b"WDBC".to_vec();
        for v in [self.rows.len(), self.fields, self.record_size, self.strings.len()] {
            out.extend_from_slice(&(v as u32).to_le_bytes());
        }
        for r in &self.rows {
            out.extend_from_slice(r);
        }
        out.extend_from_slice(&self.strings);
        out
    }

    pub fn u(row: &[u8], field: usize) -> u32 {
        row.get(field * 4..field * 4 + 4)
            .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn str_at(&self, ofs: u32) -> &str {
        let rest = self.strings.get(ofs as usize..).unwrap_or(&[]);
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        std::str::from_utf8(&rest[..end]).unwrap_or("")
    }

    pub fn s(&self, row: &[u8], field: usize) -> &str {
        self.str_at(Self::u(row, field))
    }

    pub fn find(&self, id: u32) -> Option<&Vec<u8>> {
        self.rows.iter().find(|r| Self::u(r, 0) == id)
    }

    pub fn max_id(&self) -> u32 {
        self.rows.iter().map(|r| Self::u(r, 0)).max().unwrap_or(0)
    }

    /// Append a string to the block (the empty string is offset 0), returning its offset.
    pub fn add_str(&mut self, s: &str) -> u32 {
        if s.is_empty() {
            return 0;
        }
        let at = self.strings.len() as u32;
        self.strings.extend_from_slice(s.as_bytes());
        self.strings.push(0);
        at
    }

    /// Append a row of 4-byte fields; it must have exactly this table's field count.
    pub fn push(&mut self, row: &[F]) -> Result<()> {
        ensure!(row.len() == self.fields && self.record_size == self.fields * 4, "row of {} fields for a {}-field table", row.len(), self.fields);
        let mut r = Vec::with_capacity(self.record_size);
        for f in row {
            let v = match f {
                F::U(v) => *v,
                F::S(s) => self.add_str(s),
            };
            r.extend_from_slice(&v.to_le_bytes());
        }
        self.rows.push(r);
        Ok(())
    }

    /// A row's fields as `F::U`, its strings resolved where `strings` says a column is one.
    pub fn fields_of(&self, row: &[u8], strings: &[usize]) -> Vec<F> {
        (0..self.fields)
            .map(|i| if strings.contains(&i) { F::S(self.s(row, i).to_owned()) } else { F::U(Self::u(row, i)) })
            .collect()
    }

    /// Print a row: each field as an integer, and as a string where it points at one.
    pub fn dump(&self, row: &[u8]) -> String {
        (0..self.record_size / 4)
            .map(|i| {
                let v = Self::u(row, i);
                let looks_str = v > 0
                    && (v as usize) < self.strings.len()
                    && self.strings[v as usize - 1] == 0
                    && !self.str_at(v).is_empty();
                if looks_str { format!("{i}:{v}\"{}\"", self.str_at(v)) } else if v > 0x3000_0000 && v < 0x4900_0000 { format!("{i}:{}f", f32::from_bits(v)) } else { format!("{i}:{v}") }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}