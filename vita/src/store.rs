//! The maps' packs: pieces named by their bytes under one manifest (`pocket.packs`, PocketJS's
//! `pocket3d-packs`), which every device of the game reads. This side supplies the store: a piece is a file
//! `<name>.bin`.
//!
//! A package holds the manifest and the pieces (`app0:packs`). A development build takes the manifest from
//! the computer's share and keeps the pieces on the memory card: a piece is fetched over the cable the first
//! time a pack needs it, so cooking a map again sends the pieces that changed and no others.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use duty_interface::maps::{Map, MAPS};
use duty_pack::{self as pack, Section};
use pocket3d_packs::{Manifest, Piece, ReadError, Reader, Store};

use crate::{hostfs, paths};

/// The manifest and where its pieces are.
pub struct Packs {
    manifest: Manifest,
    /// The directory the pieces are read from.
    directory: &'static str,
    /// The computer's share has pieces the directory may lack.
    fetch: bool,
}

/// A pack's pieces as files.
struct Pieces<'a> {
    directory: &'a str,
    fetch: bool,
    /// Bytes read so far, and bytes that came over the cable for them.
    read: usize,
    fetched: usize,
}

/// Copies a piece from the computer to the memory card, through a temporary file. `host0:` serves a file from
/// its start to its end and no other way (`hostfs`).
fn fetch(piece: &Piece, to: &str) -> Result<(), String> {
    let from = format!("{}/{}.bin", paths::PACKS_HOST, piece.name());
    let mut source = File::open(&from).map_err(|e| format!("{from}: {e}"))?;
    let part = format!("{to}.part");
    let mut out = File::create(&part).map_err(|e| format!("{part}: {e}"))?;
    let mut block = vec![0u8; 1024 * 1024];
    let mut done = 0usize;
    loop {
        let n = source.read(&mut block).map_err(|e| format!("{from}: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&block[..n]).map_err(|e| format!("{part}: {e}"))?;
        done += n;
    }
    drop(out);
    if done != piece.bytes as usize {
        let _ = std::fs::remove_file(&part);
        return Err(format!("{from} has {done} bytes; the manifest says {}", piece.bytes));
    }
    let _ = std::fs::remove_file(to);
    std::fs::rename(&part, to).map_err(|e| format!("{to}: {e}"))
}

impl Store for Pieces<'_> {
    type Error = String;

    fn read(&mut self, piece: &Piece, at: u32, into: &mut [u8]) -> Result<(), String> {
        let path = format!("{}/{}.bin", self.directory, piece.name());
        let open = |path: &str| File::open(path).ok().filter(|f| f.metadata().map(|m| m.len() == piece.bytes as u64).unwrap_or(false));
        let mut file = match open(&path) {
            Some(file) => file,
            None if self.fetch => {
                fetch(piece, &path)?;
                self.fetched += piece.bytes as usize;
                open(&path).ok_or(format!("{path} did not arrive whole"))?
            }
            None => return Err(format!("{path} is missing")),
        };
        file.seek(SeekFrom::Start(at as u64)).map_err(|e| format!("{path}: {e}"))?;
        file.read_exact(into).map_err(|e| format!("{path}: {e}"))?;
        self.read += into.len();
        Ok(())
    }
}

impl Packs {
    /// Finds the manifest. A development build (`live`) asks the computer first and keeps what it says on the
    /// memory card; without the cable it reads what an earlier run kept there.
    pub fn open(live: bool) -> Result<Packs, String> {
        let card = format!("{}/{}", paths::PACKS_CARD, paths::MANIFEST);
        let mut found: Option<(String, &'static str, bool)> = None;
        if live {
            let _ = std::fs::create_dir_all(paths::PACKS_CARD);
            if let Some(text) = hostfs::read(&format!("{}/{}", paths::PACKS_HOST, paths::MANIFEST), 256 * 1024).and_then(|b| String::from_utf8(b).ok()) {
                let _ = std::fs::write(&card, &text);
                found = Some((text, paths::PACKS_CARD, true));
            } else if let Ok(text) = std::fs::read_to_string(&card) {
                found = Some((text, paths::PACKS_CARD, false));
            }
        }
        if found.is_none() {
            if let Ok(text) = std::fs::read_to_string(format!("{}/{}", paths::PACKS_APP, paths::MANIFEST)) {
                found = Some((text, paths::PACKS_APP, false));
            } else if let Ok(text) = std::fs::read_to_string(&card) {
                found = Some((text, paths::PACKS_CARD, false));
            }
        }
        let (text, directory, fetch) = found.ok_or_else(|| format!("no {} in {} or {}", paths::MANIFEST, paths::PACKS_APP, paths::PACKS_CARD))?;
        let manifest = Manifest::parse(&text).map_err(|e| e.to_string())?;
        Ok(Packs { manifest, directory, fetch })
    }

    /// The maps there is a pack for, in the list's order.
    pub fn maps(&self) -> Vec<&'static Map> {
        MAPS.iter().filter(|map| self.manifest.pack(map.id).is_some()).collect()
    }

    /// Where the pieces are read from, for the status.
    pub fn directory(&self) -> &'static str {
        self.directory
    }

    /// A pack of the manifest, opened: its section table read.
    pub fn pack(&self, name: &str) -> Result<PackFile<'_>, String> {
        let pack = self.manifest.pack(name).ok_or_else(|| format!("the manifest has no pack \"{name}\""))?;
        let mut file = PackFile { reader: Reader::new(pack, Pieces { directory: self.directory, fetch: self.fetch, read: 0, fetched: 0 }), sections: Vec::new() };
        let mut head = [0u8; 16];
        file.range(0, &mut head)?;
        let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        if word(0) != pack::MAGIC {
            return Err(format!("the pack \"{name}\" is not one of this game's"));
        }
        if word(4) != pack::VERSION {
            return Err(format!("the pack \"{name}\" is version {} and this program reads version {}: cook it again", word(4), pack::VERSION));
        }
        let count = word(8) as usize;
        let mut table = vec![0u8; count * 16];
        file.range(16, &mut table)?;
        file.sections = (0..count).filter_map(|i| pack::read::<Section>(&table, i * 16)).collect();
        Ok(file)
    }

    /// A whole small pack (a map's picture from above), or none when the manifest has no such pack.
    pub fn whole(&self, name: &str) -> Option<Vec<u8>> {
        let pack = self.manifest.pack(name)?;
        let mut bytes = vec![0u8; pack.bytes as usize];
        let mut reader = Reader::new(pack, Pieces { directory: self.directory, fetch: self.fetch, read: 0, fetched: 0 });
        reader.read(0, &mut bytes).ok()?;
        Some(bytes)
    }
}

/// One pack, read a section or a part of one at a time: nothing of it is held here.
pub struct PackFile<'a> {
    reader: Reader<'a, Pieces<'a>>,
    sections: Vec<Section>,
}

impl PackFile<'_> {
    fn range(&mut self, offset: u64, into: &mut [u8]) -> Result<(), String> {
        self.reader.read(offset, into).map_err(|e| match e {
            ReadError::Pack(e) => e.to_string(),
            ReadError::Store(e) => e,
        })
    }

    /// The pack's size, the bytes read of it so far, and of those the bytes that came over the cable.
    pub fn progress(&self) -> (usize, usize, usize) {
        (self.reader.pack.bytes as usize, self.reader.store.read, self.reader.store.fetched)
    }

    fn section(&self, tag: u32) -> Result<Section, String> {
        self.sections.iter().copied().find(|s| s.tag == tag).ok_or_else(|| format!("the pack has no {} section", String::from_utf8_lossy(&tag.to_le_bytes())))
    }

    pub fn has(&self, tag: u32) -> bool {
        self.sections.iter().any(|s| s.tag == tag)
    }

    pub fn size(&self, tag: u32) -> Result<usize, String> {
        Ok(self.section(tag)?.size as usize)
    }

    /// `into.len()` bytes of a section, from `offset` in it.
    pub fn read_into(&mut self, tag: u32, offset: usize, into: &mut [u8]) -> Result<(), String> {
        let section = self.section(tag)?;
        if offset + into.len() > section.size as usize {
            return Err(format!("a read past the end of the {} section", String::from_utf8_lossy(&tag.to_le_bytes())));
        }
        self.range(section.offset as u64 + offset as u64, into)
    }

    /// A whole section.
    pub fn read(&mut self, tag: u32) -> Result<Vec<u8>, String> {
        let mut bytes = vec![0u8; self.size(tag)?];
        self.read_into(tag, 0, &mut bytes)?;
        Ok(bytes)
    }

    /// A section as records.
    pub fn records<T: Copy>(&mut self, tag: u32) -> Result<Vec<T>, String> {
        let size = self.size(tag)?;
        let each = core::mem::size_of::<T>();
        if each == 0 || size % each != 0 {
            return Err(format!("the {} section is not whole records", String::from_utf8_lossy(&tag.to_le_bytes())));
        }
        let mut records: Vec<T> = Vec::with_capacity(size / each);
        // SAFETY: the records are plain data read from the pack; the slice covers the vector's whole room, and
        // its length is set once every byte of it is written.
        unsafe {
            self.read_into(tag, 0, core::slice::from_raw_parts_mut(records.as_mut_ptr() as *mut u8, size))?;
            records.set_len(size / each);
        }
        Ok(records)
    }
}
