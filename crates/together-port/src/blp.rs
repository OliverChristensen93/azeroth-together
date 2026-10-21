//! BLP textures resized for 1.12. WotLK draws a character's skin at twice 1.12's size (a 512 by
//! 512 body, 256 by 128 faces), and 1.12's compositor paints its layers at fixed pixels for a
//! 256 by 256 body, so every composited layer is halved. Written uncompressed (BLP2 Raw3, BGRA8)
//! with a full mip chain, which `benilla-blp` decodes as is.

use anyhow::{Context, Result};

/// The BLP2 header: magic, content, compression, alpha bits, alpha type, has-mips, size, 16 mip
/// offsets, 16 mip sizes; a 256-entry palette always follows it.
const HEADER: usize = 148;
const PALETTE: usize = 1024;

/// One RGBA8 image.
struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Half the size, each pixel the mean of the 2 by 2 it covers (a side of 1 stays 1).
fn half(img: &Image) -> Image {
    let (w, h) = ((img.width / 2).max(1), (img.height / 2).max(1));
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let mut sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(img.width - 1);
                    let sy = (y * 2 + dy).min(img.height - 1);
                    sum += u32::from(img.rgba[((sy * img.width + sx) * 4 + c) as usize]);
                }
                rgba[((y * w + x) * 4 + c) as usize] = ((sum + 2) / 4) as u8;
            }
        }
    }
    Image { width: w, height: h, rgba }
}

/// A BLP2 Raw3 file of `top` and its mip chain down to 1 by 1.
fn encode(top: Image) -> Vec<u8> {
    let mut levels = vec![top];
    while levels.len() < 16 {
        let last = levels.last().unwrap();
        if last.width == 1 && last.height == 1 {
            break;
        }
        let next = half(last);
        levels.push(next);
    }
    let mut out = vec![0u8; HEADER + PALETTE];
    out[0..4].copy_from_slice(b"BLP2");
    out[4..8].copy_from_slice(&1u32.to_le_bytes()); // Direct
    out[8] = 3; // Raw3
    out[9] = 8; // alpha bits
    out[10] = 0;
    out[11] = 1; // has mips
    out[12..16].copy_from_slice(&levels[0].width.to_le_bytes());
    out[16..20].copy_from_slice(&levels[0].height.to_le_bytes());
    for (i, l) in levels.iter().enumerate() {
        let at = out.len() as u32;
        for px in l.rgba.as_chunks::<4>().0 {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let size = out.len() as u32 - at;
        out[20 + i * 4..24 + i * 4].copy_from_slice(&at.to_le_bytes());
        out[84 + i * 4..88 + i * 4].copy_from_slice(&size.to_le_bytes());
    }
    out
}

/// A BLP at half its size.
pub fn halved(bytes: &[u8]) -> Result<Vec<u8>> {
    let d = benilla_blp::decode(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    let top = d.mips.into_iter().next().context("a BLP without levels")?;
    Ok(encode(half(&Image { width: top.width, height: top.height, rgba: top.rgba })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_halved_texture_decodes_at_half_size_with_its_mean_colour() {
        // 4 by 2: a red and a blue 2 by 2 block, half transparent.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            rgba.extend_from_slice(&[255, 0, 0, 128, 255, 0, 0, 128, 0, 0, 255, 128, 0, 0, 255, 128]);
        }
        let file = encode(half(&Image { width: 4, height: 2, rgba }));
        let d = benilla_blp::decode(&file).expect("benilla-blp reads it");
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(d.mips[0].rgba, vec![255, 0, 0, 128, 0, 0, 255, 128]);
    }
}