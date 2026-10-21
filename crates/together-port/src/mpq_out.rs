//! A 1.12-style MPQ writer: format v1, 4 KiB sectors, every file sectored and zlib-compressed
//! (never single-unit, never encrypted), with a `(listfile)`, which is the envelope
//! `benilla-mpq` reads.

use std::io::Write;
use std::path::Path;

use anyhow::{ensure, Result};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use wow_mpq::crypto::{encrypt_block, hash_string, hash_type};

const SECTOR_SHIFT: u16 = 3;
const SECTOR: usize = 512 << SECTOR_SHIFT;
const FLAG_EXISTS: u32 = 0x8000_0000;
const FLAG_COMPRESS: u32 = 0x0000_0200;

/// The files of an archive to be written, by internal path.
#[derive(Default)]
pub struct MpqOut {
    files: Vec<(String, Vec<u8>)>,
}

impl MpqOut {
    /// Add (or replace) a file.
    pub fn add(&mut self, name: &str, data: Vec<u8>) {
        let name = name.replace('/', "\\");
        match self.files.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            Some(f) => f.1 = data,
            None => self.files.push((name, data)),
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        let name = name.replace('/', "\\");
        self.files.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name))
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn write(mut self, path: &Path) -> Result<()> {
        let listfile: String = self.files.iter().map(|(n, _)| format!("{n}\r\n")).collect();
        self.files.push(("(listfile)".into(), listfile.into_bytes()));
        ensure!(self.files.iter().all(|(_, d)| !d.is_empty()), "an empty file");

        let mut out = vec![0u8; 32];
        let mut blocks: Vec<[u32; 4]> = Vec::with_capacity(self.files.len());
        for (_, data) in &self.files {
            let pos = out.len();
            let sectors: Vec<&[u8]> = data.chunks(SECTOR).collect();
            let mut table = Vec::with_capacity(sectors.len() + 1);
            let mut body = Vec::new();
            let head = (sectors.len() + 1) * 4;
            for s in &sectors {
                table.push((head + body.len()) as u32);
                let mut e = ZlibEncoder::new(vec![0x02u8], Compression::best());
                e.write_all(s)?;
                let packed = e.finish()?;
                // A sector no smaller packed is stored raw; the reader knows it by its size.
                body.extend_from_slice(if packed.len() < s.len() { &packed } else { s });
            }
            table.push((head + body.len()) as u32);
            for t in &table {
                out.extend_from_slice(&t.to_le_bytes());
            }
            out.extend_from_slice(&body);
            blocks.push([pos as u32, (out.len() - pos) as u32, data.len() as u32, FLAG_EXISTS | FLAG_COMPRESS]);
        }

        let hash_size = (self.files.len() * 2).next_power_of_two().max(16);
        let mut hash = vec![[u32::MAX; 4]; hash_size];
        for (i, (name, _)) in self.files.iter().enumerate() {
            let mut slot = hash_string(name, hash_type::TABLE_OFFSET) as usize % hash_size;
            while hash[slot][3] != u32::MAX {
                slot = (slot + 1) % hash_size;
            }
            // name A, name B, locale 0 + platform 0, block index.
            hash[slot] = [hash_string(name, hash_type::NAME_A), hash_string(name, hash_type::NAME_B), 0, i as u32];
        }

        let mut words: Vec<u32> = hash.iter().flatten().copied().collect();
        encrypt_block(&mut words, hash_string("(hash table)", hash_type::FILE_KEY));
        let hash_pos = out.len();
        for w in &words {
            out.extend_from_slice(&w.to_le_bytes());
        }
        let mut words: Vec<u32> = blocks.iter().flatten().copied().collect();
        encrypt_block(&mut words, hash_string("(block table)", hash_type::FILE_KEY));
        let block_pos = out.len();
        for w in &words {
            out.extend_from_slice(&w.to_le_bytes());
        }

        let mut h = Vec::with_capacity(32);
        h.extend_from_slice(b"MPQ\x1a");
        h.extend_from_slice(&32u32.to_le_bytes());
        h.extend_from_slice(&(out.len() as u32).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(&SECTOR_SHIFT.to_le_bytes());
        h.extend_from_slice(&(hash_pos as u32).to_le_bytes());
        h.extend_from_slice(&(block_pos as u32).to_le_bytes());
        h.extend_from_slice(&(hash_size as u32).to_le_bytes());
        h.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
        out[..32].copy_from_slice(&h);

        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &out)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}