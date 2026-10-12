//! The pack: what Pocket Duty's compiler writes and its runtimes read.
//!
//! A pack is a table of sections. A section is an array of one record type,
//! in the byte order and layout of the device that reads it (every device so
//! far is little-endian with 4-byte floats), so a runtime reads a section
//! straight into memory and uses it.
//!
//! ```text
//! u32 MAGIC · u32 VERSION · u32 sections · u32 0
//! Section × sections
//! the sections' bytes, each on a 64-byte boundary
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

pub mod figure;
pub mod sound;

pub const MAGIC: u32 = u32::from_le_bytes(*b"DUTY");
/// Raised by any change to a record or to what a section holds.
pub const VERSION: u32 = 1;

pub const fn tag(name: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*name)
}

/// One [`Scene`].
pub const SCEN: u32 = tag(b"SCEN");
/// [`Texture`] records, and the texels they point into.
pub const TEXT: u32 = tag(b"TEXT");
pub const TEXD: u32 = tag(b"TEXD");
/// The world: [`Chunk`] records, their vertices ([`WorldVertex`]) and indices (`u16`).
pub const CHNK: u32 = tag(b"CHNK");
pub const WVTX: u32 = tag(b"WVTX");
pub const WIDX: u32 = tag(b"WIDX");
/// [`Cluster`] records: the runs of triangles a level of detail is drawn by.
pub const CLUS: u32 = tag(b"CLUS");
/// What can be seen from where: one [`ViewCell`] per cell of the grid, the chunk numbers (`u16`)
/// their lists hold, and their bits, one per cluster.
pub const VCEL: u32 = tag(b"VCEL");
pub const VLST: u32 = tag(b"VLST");
pub const VBIT: u32 = tag(b"VBIT");
/// The collision world: positions (`[f32; 3]`), triangles and nodes (`duty_sim::bvh`).
pub const COLV: u32 = tag(b"COLV");
pub const COLT: u32 = tag(b"COLT");
pub const COLN: u32 = tag(b"COLN");
/// The soldiers and the held weapons: models, clips and their points (`figure`).
pub const FIGS: u32 = tag(b"FIGS");
/// The sounds' records and their samples (`sound`).
pub const SNDS: u32 = tag(b"SNDS");
pub const SNDD: u32 = tag(b"SNDD");
/// The weapons' numbers and the walk grid (`duty_sim`).
pub const WEAP: u32 = tag(b"WEAP");
pub const NAVG: u32 = tag(b"NAVG");
/// The compile receipt's identity: the text of `receipt.json`'s `id`.
pub const META: u32 = tag(b"META");

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Section {
    pub tag: u32,
    pub offset: u32,
    pub size: u32,
    pub pad: u32,
}

/// The header and the table: 16 bytes, then 16 per section.
pub const fn table_bytes(sections: usize) -> usize {
    16 + sections * 16
}

/// What holds for the whole pack.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Scene {
    /// The world's box, metres. Cells are counted from `min` along x and z.
    pub min: [f32; 3],
    /// Metres along a cell's side.
    pub cell: f32,
    pub max: [f32; 3],
    /// Metres a step of a stored position is. Every position lies on one lattice of this
    /// spacing, so two chunks that share a corner store the same point.
    pub quantum: f32,
    pub cells_x: u32,
    pub cells_z: u32,
    /// The height the stored positions' vertical zero stands at.
    pub origin_y: f32,
    /// Metres from the eye at which a chunk goes to its second level of detail, and to its third.
    pub lod: [f32; 2],
    /// Unit vector toward the sun.
    pub sun: [f32; 3],
    /// The sun's colour and the sky's over a soldier or a weapon, as display colours 0…1.
    pub sun_colour: [f32; 3],
    pub sky_colour: [f32; 3],
    /// The haze: where it starts and where it is complete, metres, and its colour (ABGR).
    pub haze: [f32; 2],
    pub haze_colour: u32,
    /// The ground's shadow picture: its texture, and the rows that take a world position to its
    /// coordinates (`u = x·a + z·b + c`, then `v`).
    pub shadow_texture: u32,
    pub shadow_rows: [[f32; 3]; 2],
    /// The sky: the first of its textures, how many stand side by side around the horizon, and
    /// the sine of the height their top edge is at (their bottom edge is at `sky_floor`).
    pub sky_texture: u32,
    pub sky_columns: u32,
    pub sky_top: f32,
    pub sky_floor: f32,
    /// The colour of the sky above the pictures' top edge (ABGR).
    pub zenith: u32,
    /// Where a match starts: the feet, and the yaw.
    pub start: [f32; 4],
    /// The effects' picture: four tiles in a square, each a shape in white whose palette entry
    /// `i` is as opaque as `i`: a soft dot, a puff, a flash, a round's hole.
    pub fx_texture: u32,
    /// The level's own origin and its horizontal axes in the world (the street runs along the second).
    pub level_origin: [f32; 3],
    pub level_x: [f32; 3],
    pub level_z: [f32; 3],
}

pub mod texture {
    /// 8-bit indices into a palette of 256 32-bit colours; texels swizzled.
    pub const T8: u8 = 0;
    /// Palette entry 0 is clear: the device tests alpha.
    pub const CUTOUT: u16 = 1;
    /// Coordinates outside 0…1 take the edge's texel.
    pub const CLAMP: u16 = 2;
    /// Bytes of a palette.
    pub const PALETTE_BYTES: usize = 1024;
}

/// A picture: at `offset` of the texel section, 16-byte aligned, a palette, then each level from
/// the largest, each 16-byte aligned.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Texture {
    pub offset: u32,
    pub width: u16,
    pub height: u16,
    pub levels: u8,
    pub format: u8,
    pub flags: u16,
}

impl Texture {
    /// Bytes of level `l`, and where it starts from `offset`.
    pub fn level(&self, l: u32) -> (usize, usize) {
        let mut at = texture::PALETTE_BYTES;
        for k in 0..l {
            at += (((self.width as usize >> k) * (self.height as usize >> k)) + 15) & !15;
        }
        (at, (self.width as usize >> l) * (self.height as usize >> l))
    }
}

/// A world vertex in the GE's component order: texture, colour, position.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct WorldVertex {
    /// `uv / 32768 × chunk.uv_scale + chunk.uv_offset` are the texture coordinates.
    pub uv: [i16; 2],
    /// Five bits of red, six of green, five of blue: the surface's colour under the baked light,
    /// for a texel of its tile's mean colour. The device shows texel × colour × 2, and such a
    /// texel is one half.
    pub colour: u16,
    /// Steps of `Scene::quantum` from the cell's centre.
    pub pos: [i16; 3],
}

pub mod chunk {
    /// Flat ground: drawn once more with the shadow picture.
    pub const GROUND: u16 = 1;
    /// Seen from both sides.
    pub const TWO_SIDED: u16 = 2;
    /// Glass: mixed over what is behind it by `Chunk::opacity`.
    pub const BLEND: u16 = 4;
    /// The texture's clear texels are holes.
    pub const CUTOUT: u16 = 8;
    /// Small things: only the first level of detail has them.
    pub const DETAIL: u16 = 16;
    pub const LODS: usize = 3;
}

/// One level of detail of a chunk: where its vertices and indices start, how many indices there
/// are, and the clusters that cut the index list into runs.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Lod {
    pub vertex: u32,
    pub index: u32,
    pub indices: u32,
    pub cluster: u32,
    pub clusters: u32,
}

/// A run of a level's triangles that lie together and face alike: what the device decides to
/// draw or to leave out.
///
/// The GE clips at the near plane and nowhere else: a triangle with a corner outside its guard
/// band is not drawn at all. Only a triangle with a long edge can reach from inside the view to
/// outside the band, so a cluster's `large` triangles come first in its run, and when the
/// cluster's sphere is not wholly inside the band the device tests those and cuts them itself
/// (`duty_sim::clip`).
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Cluster {
    /// A sphere around its triangles.
    pub centre: [f32; 3],
    pub radius: f32,
    /// Its faces' common direction, each component over 127, and the cosine (over 127) the eye's
    /// direction makes with it at most when any face is turned to the eye. 127: seen from anywhere.
    pub axis: [i8; 3],
    pub cutoff: i8,
    /// Where its run starts in the level's index list, and how many indices it has.
    pub first: u32,
    pub count: u16,
    /// Triangles at the start of the run with an edge long enough to leave the guard band.
    pub large: u16,
    /// Half the sides of a box about `centre` that holds its triangles, each over 255 of `radius`.
    /// The box lies along the level's own axes (`Scene::level_x`, up, `Scene::level_z`), as the
    /// walls do: a wall's box is thin where its sphere is not.
    pub half: [u8; 3],
    pub pad: u8,
}

impl Cluster {
    pub fn half(&self) -> [f32; 3] {
        let s = self.radius * (1.0 / 255.0);
        [self.half[0] as f32 * s, self.half[1] as f32 * s, self.half[2] as f32 * s]
    }
}

/// What one draw holds: the triangles of one texture in one cell.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Chunk {
    pub min: [f32; 3],
    pub texture: u16,
    pub flags: u16,
    pub max: [f32; 3],
    /// The cell whose centre its positions count from: `z × cells_x + x`.
    pub cell: u16,
    /// 0…255: how much of a `BLEND` chunk covers what is behind it.
    pub opacity: u8,
    pub pad: u8,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    pub lod: [Lod; chunk::LODS],
}

/// What can be seen from inside a cell: `count` chunk numbers from `first` of the list section,
/// and at `bits` of the bit section one bit per cluster of the pack, set for a cluster some
/// place in the cell sees. A cell no eye can be in has `count` 0.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ViewCell {
    pub first: u32,
    pub count: u32,
    pub bits: u32,
}

impl Scene {
    /// The cell a point is in, if it is inside the grid.
    pub fn cell_at(&self, x: f32, z: f32) -> Option<u32> {
        let (i, j) = (((x - self.min[0]) / self.cell) as i32, ((z - self.min[2]) / self.cell) as i32);
        (x >= self.min[0] && z >= self.min[2] && i < self.cells_x as i32 && j < self.cells_z as i32).then_some(j as u32 * self.cells_x + i as u32)
    }

    /// The point a cell's stored positions count from.
    pub fn cell_centre(&self, cell: u32) -> [f32; 3] {
        let (i, j) = (cell % self.cells_x, cell / self.cells_x);
        [self.min[0] + (i as f32 + 0.5) * self.cell, self.origin_y, self.min[2] + (j as f32 + 0.5) * self.cell]
    }
}

/// Reads a record at `offset` of a section's bytes. `T` is plain data.
pub fn read<T: Copy>(bytes: &[u8], offset: usize) -> Option<T> {
    if offset + core::mem::size_of::<T>() > bytes.len() {
        return None;
    }
    // SAFETY: the range is inside `bytes`, and every record is plain data.
    Some(unsafe { core::ptr::read_unaligned(bytes.as_ptr().add(offset) as *const T) })
}

/// A section's place in a pack held whole in memory.
pub fn section(pack: &[u8], want: u32) -> Option<&[u8]> {
    let head: [u32; 4] = read(pack, 0)?;
    if head[0] != MAGIC || head[1] != VERSION {
        return None;
    }
    for i in 0..head[2] as usize {
        let s: Section = read(pack, 16 + i * 16)?;
        if s.tag == want {
            return pack.get(s.offset as usize..(s.offset + s.size) as usize);
        }
    }
    None
}

#[cfg(feature = "std")]
mod write {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// Collects sections and lays the pack out.
    #[derive(Default)]
    pub struct Writer {
        sections: Vec<(u32, Vec<u8>)>,
    }

    /// The bytes of a slice of plain records.
    pub fn bytes_of<T: Copy>(records: &[T]) -> &[u8] {
        // SAFETY: plain data, read as bytes.
        unsafe { core::slice::from_raw_parts(records.as_ptr() as *const u8, core::mem::size_of_val(records)) }
    }

    impl Writer {
        pub fn add<T: Copy>(&mut self, tag: u32, records: &[T]) {
            self.sections.push((tag, bytes_of(records).to_vec()));
        }

        pub fn finish(self) -> Vec<u8> {
            let mut at = (table_bytes(self.sections.len()) + 63) & !63;
            let mut out = Vec::new();
            for word in [MAGIC, VERSION, self.sections.len() as u32, 0] {
                out.extend_from_slice(&word.to_le_bytes());
            }
            for (tag, bytes) in &self.sections {
                out.extend_from_slice(bytes_of(&[Section { tag: *tag, offset: at as u32, size: bytes.len() as u32, pad: 0 }]));
                at = (at + bytes.len() + 63) & !63;
            }
            for (_, bytes) in &self.sections {
                out.resize((out.len() + 63) & !63, 0);
                out.extend_from_slice(bytes);
            }
            out
        }
    }
}

#[cfg(feature = "std")]
pub use write::{bytes_of, Writer};

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn records_keep_their_sizes() {
        assert_eq!(core::mem::size_of::<WorldVertex>(), 12);
        assert_eq!(core::mem::size_of::<Texture>(), 12);
        assert_eq!(core::mem::size_of::<Lod>(), 20);
        assert_eq!(core::mem::size_of::<Cluster>(), 32);
        assert_eq!(core::mem::size_of::<Chunk>(), 108);
        assert_eq!(core::mem::size_of::<Scene>() % 4, 0);
    }

    #[test]
    fn a_pack_gives_its_sections_back() {
        let mut w = Writer::default();
        w.add(SCEN, &[Scene { cell: 8.0, cells_x: 3, cells_z: 2, ..Default::default() }]);
        w.add(WIDX, &[1u16, 2, 3]);
        let pack = w.finish();
        let scene: Scene = read(section(&pack, SCEN).unwrap(), 0).unwrap();
        assert_eq!((scene.cell, scene.cells_x), (8.0, 3));
        assert_eq!(section(&pack, WIDX).unwrap(), &[1, 0, 2, 0, 3, 0]);
        assert!(section(&pack, CHNK).is_none());
        assert_eq!(section(&pack, WIDX).unwrap().as_ptr() as usize % 64, pack.as_ptr() as usize % 64);
    }
}
