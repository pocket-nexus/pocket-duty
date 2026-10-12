//! Reading the pack: the PSP's (`profiles/psp30.json`), whole, a range at a
//! time so the interface can say how much has arrived; its sections as
//! records; and its pictures, which the GE reads as 8-bit indices into a
//! palette in its own tiled order, as rows of RGBA every WebGPU device samples.

use std::cell::Cell;
use std::rc::Rc;

use duty_pack::{self as pack, Texture};
use pocket_web_wgpu::source::Source;

/// Bytes asked for in one read. A pack cut into pieces fetches the pieces of a read all at once.
const READ: u64 = 8 << 20;

/// How much of the pack has arrived, for the loading screen.
#[derive(Clone, Default)]
pub struct Progress(Rc<Cell<(u64, u64)>>);

impl Progress {
    /// Bytes read so far, and the pack's size (0 until it is known).
    pub fn get(&self) -> (u64, u64) {
        self.0.get()
    }
}

/// The whole pack from `source`.
pub async fn read(source: &Source, progress: &Progress) -> Result<Vec<u8>, String> {
    let length = source.length().await?;
    progress.0.set((0, length));
    let mut bytes = Vec::with_capacity(length as usize);
    while (bytes.len() as u64) < length {
        let at = bytes.len() as u64;
        bytes.extend_from_slice(&source.range(at, READ.min(length - at)).await?);
        progress.0.set((bytes.len() as u64, length));
    }
    Ok(bytes)
}

fn name(tag: u32) -> String {
    String::from_utf8_lossy(&tag.to_le_bytes()).into_owned()
}

/// A section's bytes.
pub fn section(bytes: &[u8], tag: u32) -> Result<&[u8], String> {
    pack::section(bytes, tag).ok_or_else(|| format!("the pack has no {} section (this renderer reads the PSP's pack, version {})", name(tag), pack::VERSION))
}

/// A section as records, copied to where they are aligned.
pub fn records<T: Copy>(bytes: &[u8], tag: u32) -> Result<Vec<T>, String> {
    let s = section(bytes, tag)?;
    let size = core::mem::size_of::<T>();
    Ok((0..s.len() / size).map(|i| pack::read::<T>(s, i * size).unwrap()).collect())
}

/// Records that live as long as the program: the match borrows the collision world and the walk grid.
pub fn kept<T: Copy>(bytes: &[u8], tag: u32) -> Result<&'static [T], String> {
    Ok(Box::leak(records::<T>(bytes, tag)?.into_boxed_slice()))
}

/// The bytes of plain records.
pub fn bytes_of<T: Copy>(records: &[T]) -> &[u8] {
    // SAFETY: plain data, read as bytes.
    unsafe { core::slice::from_raw_parts(records.as_ptr() as *const u8, core::mem::size_of_val(records)) }
}

/// A picture's size at a level.
pub fn level_size(record: &Texture, level: u32) -> (u32, u32) {
    ((record.width as u32 >> level).max(1), (record.height as u32 >> level).max(1))
}

/// One level of a picture as rows of RGBA, the top row first. `texels` is the pack's `TEXD` section.
///
/// The pack holds a level as the GE reads it: one byte a texel, in tiles of 16 texels by 8 rows,
/// each byte the number of one of the palette's 256 colours.
pub fn rgba(record: &Texture, texels: &[u8], level: u32) -> Result<Vec<u8>, String> {
    let (w, h) = level_size(record, level);
    let (w, h) = (w as usize, h as usize);
    let base = record.offset as usize;
    let (at, _) = record.level(level);
    let (stride, rows) = ((w + 15) & !15, (h + 7) & !7);
    let palette = texels.get(base..base + pack::texture::PALETTE_BYTES).ok_or("a picture's palette is cut short")?;
    let tiles = texels.get(base + at..base + at + stride * rows).ok_or("a picture's texels are cut short")?;
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let index = tiles[((y / 8) * (stride / 16) + x / 16) * 128 + (y % 8) * 16 + x % 16] as usize;
            out[(y * w + x) * 4..][..4].copy_from_slice(&palette[index * 4..index * 4 + 4]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tiled_level_comes_back_as_rows() {
        // A picture of 32 by 16 texels: texel (x, y) holds the colour number `x + y`, and colour
        // `i` of the palette is (i, 2 i, 3 i, 255).
        let (w, h) = (32usize, 16usize);
        let mut section = Vec::new();
        for i in 0..256u32 {
            section.extend_from_slice(&[i as u8, (2 * i) as u8, (3 * i) as u8, 255]);
        }
        let mut tiles = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                tiles[((y / 8) * (w / 16) + x / 16) * 128 + (y % 8) * 16 + x % 16] = (x + y) as u8;
            }
        }
        section.extend_from_slice(&tiles);
        let record = Texture { offset: 0, width: w as u16, height: h as u16, levels: 1, format: pack::texture::T8, flags: 0 };
        let px = rgba(&record, &section, 0).unwrap();
        for (x, y) in [(0, 0), (17, 3), (31, 15), (16, 8)] {
            let i = (x + y) as u32;
            assert_eq!(&px[(y * w + x) * 4..][..4], &[i as u8, (2 * i) as u8, (3 * i) as u8, 255]);
        }
        assert!(rgba(&record, &section[..section.len() - 1], 0).is_err());
    }
}
