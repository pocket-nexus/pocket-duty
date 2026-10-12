//! Reading the DutyIR: a `.dir` file is a JSON header and the typed arrays it names
//! (`web/src/pocket/bundle.js` writes them).

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

pub struct Dir {
    pub meta: Value,
    arrays: HashMap<String, (String, usize, usize)>,
    data: Vec<u8>,
}

fn element_bytes(kind: &str) -> usize {
    match kind {
        "f32" | "u32" | "i32" => 4,
        "u16" | "i16" => 2,
        _ => 1,
    }
}

impl Dir {
    pub fn open(path: &Path) -> Result<Dir, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e} (run `bun tools/duty.ts export` first)", path.display()))?;
        if bytes.len() < 8 || &bytes[..4] != b"DIR1" {
            return Err(format!("{} is not a DutyIR file", path.display()));
        }
        let head = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let meta: Value = serde_json::from_slice(&bytes[8..8 + head]).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut arrays = HashMap::new();
        for (name, a) in meta["arrays"].as_object().ok_or("the header lists no arrays")? {
            arrays.insert(name.clone(), (a["type"].as_str().unwrap_or("u8").to_string(), a["offset"].as_u64().unwrap_or(0) as usize, a["length"].as_u64().unwrap_or(0) as usize));
        }
        Ok(Dir { meta, arrays, data: bytes[8 + head..].to_vec() })
    }

    fn raw(&self, name: &str, kind: &str) -> &[u8] {
        let (have, offset, length) = self.arrays.get(name).unwrap_or_else(|| panic!("the IR has no array {name}"));
        assert_eq!(have, kind, "array {name} holds {have}");
        &self.data[*offset..*offset + *length * element_bytes(kind)]
    }

    pub fn has(&self, name: &str) -> bool {
        self.arrays.contains_key(name)
    }

    pub fn f32s(&self, name: &str) -> Vec<f32> {
        self.raw(name, "f32").chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
    }

    pub fn u32s(&self, name: &str) -> Vec<u32> {
        self.raw(name, "u32").chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
    }

    pub fn u16s(&self, name: &str) -> Vec<u16> {
        self.raw(name, "u16").chunks_exact(2).map(|c| u16::from_le_bytes(c.try_into().unwrap())).collect()
    }

    pub fn u8s(&self, name: &str) -> &[u8] {
        self.raw(name, "u8")
    }

    pub fn i8s(&self, name: &str) -> Vec<i8> {
        self.raw(name, "i8").iter().map(|b| *b as i8).collect()
    }
}
