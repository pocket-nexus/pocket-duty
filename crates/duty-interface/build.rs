//! Writes the maps' table (`maps/maps.json`, the one list of them) as Rust for `src/maps.rs`.
//!
//! The file is ours and flat: an array of objects whose values are short strings and `true`. It is read
//! with a scanner of a few lines, so a device's build needs no JSON crate.

use std::fmt::Write;
use std::path::PathBuf;

/// The string value of `"key"` in one object's text.
fn value<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let at = object.find(&format!("\"{key}\""))?;
    let rest = object[at + key.len() + 2..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

fn main() {
    let list = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../maps/maps.json");
    println!("cargo:rerun-if-changed={}", list.display());
    let text = std::fs::read_to_string(&list).unwrap_or_else(|e| panic!("{}: {e}", list.display()));
    let mut out = String::from("/// The maps, in the order `maps/maps.json` lists them. The first is the reference's own street.\npub const MAPS: &[Map] = &[\n");
    let mut count = 0;
    for object in text.split('{').skip(1) {
        let object = object.split('}').next().unwrap();
        let (id, title) = (value(object, "id").expect("a map's id"), value(object, "title").expect("a map's title"));
        assert!(!id.is_empty() && id.len() <= 24 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "maps.json: {id} is not a map's id");
        assert!(!title.contains('\\'), "maps.json: a title is plain text");
        writeln!(out, "    Map {{ id: {id:?}, title: {title:?} }},").unwrap();
        count += 1;
    }
    assert!(count > 0, "maps.json lists no map");
    out.push_str("];\n");
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("maps.rs"), out).unwrap();
}
