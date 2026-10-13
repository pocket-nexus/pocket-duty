//! The maps a match is played on: the street and a directory a map under `maps/`, compiled in (`build.rs`).
//!
//! A device holds one map's pack at a time. It says which maps it has a pack for
//! ([`crate::Session::maps`]) and which one it holds ([`crate::Session::map_id`]); the interface lists
//! the first and asks for one of them with [`crate::Command::Map`].

/// One map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Map {
    /// Lowercase letters, digits and hyphens: the name the interface asks for it by.
    pub id: &'static str,
    /// What the interface calls it.
    pub title: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/maps.rs"));

/// The map of that id.
pub fn named(id: &str) -> Option<&'static Map> {
    MAPS.iter().find(|m| m.id == id)
}

/// The first map: the reference's own street, which every device has.
pub fn first() -> &'static Map {
    &MAPS[0]
}

impl Map {
    /// The name of this map's pack beside a program: the street's keeps the name it had when it was the
    /// only one (`duty.pack`), another map's is its id (`yards.pack`). Written into `out` with a NUL
    /// after it; returns the name's length. `out` holds 32 bytes or more.
    pub fn pack_name(&self, out: &mut [u8]) -> usize {
        let stem: &[u8] = if self.id == first().id { b"duty" } else { self.id.as_bytes() };
        let mut at = 0;
        for part in [stem, b".pack\0"] {
            out[at..at + part.len()].copy_from_slice(part);
            at += part.len();
        }
        at - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_the_directories() {
        // Every directory under maps/ with a `map.json` is here, after the street.
        let maps = concat!(env!("CARGO_MANIFEST_DIR"), "/../../maps");
        let mut ids: std::vec::Vec<std::string::String> = std::fs::read_dir(maps).unwrap().flatten().filter(|e| e.path().join("map.json").exists()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        ids.sort();
        let mut listed: std::vec::Vec<&str> = MAPS.iter().skip(1).map(|m| m.id).collect();
        listed.sort();
        assert_eq!(ids, listed);
        assert_eq!(first().id, "street");
        assert!(named("street").is_some() && named("no-such-map").is_none());
    }

    #[test]
    fn a_pack_is_named_by_its_map() {
        let mut name = [0u8; 32];
        let n = first().pack_name(&mut name);
        assert_eq!(&name[..=n], b"duty.pack\0");
        if let Some(other) = MAPS.get(1) {
            let n = other.pack_name(&mut name);
            assert_eq!(&name[..n], [other.id.as_bytes(), b".pack"].concat().as_slice());
            assert_eq!(name[n], 0);
        }
    }
}
