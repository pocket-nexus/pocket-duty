//! Writes the maps' table as Rust for `src/maps.rs`: the reference's street (`maps/street.json`), then
//! every directory under `maps/` that has a `map.json`. No file lists the maps.
//!
//! The files are ours and flat: objects whose values are short strings and a number. They are read with
//! a scanner of a few lines, so a device's build needs no JSON crate.

use std::fmt::Write;
use std::path::PathBuf;

/// The string value of `"key"` in one object's text.
fn value<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let at = object.find(&format!("\"{key}\""))?;
    let rest = object[at + key.len() + 2..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

fn main() {
    let maps = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../maps");
    // (a map added or taken away is a change of the directory itself)
    println!("cargo:rerun-if-changed={}", maps.display());
    let read = |path: &PathBuf| {
        println!("cargo:rerun-if-changed={}", path.display());
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    // The reference's street first; then a directory a map, by its `order` and then its name.
    let street = read(&maps.join("street.json"));
    let mut listed = vec![(-1i64, value(&street, "id").expect("the street's id").to_string(), value(&street, "title").expect("the street's title").to_string())];
    for entry in std::fs::read_dir(&maps).expect("maps/").flatten() {
        let file = entry.path().join("map.json");
        if !file.exists() {
            continue;
        }
        let text = read(&file);
        let order = text.find("\"order\"").and_then(|at| text[at + 7..].trim_start().strip_prefix(':')).and_then(|rest| rest.trim_start().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).unwrap_or(1000);
        listed.push((order, entry.file_name().to_string_lossy().into_owned(), value(&text, "title").expect("a map's title").to_string()));
    }
    listed.sort();
    let mut out = String::from("/// The maps: the reference's own street, then `maps/<id>/` by its `map.json`.\npub const MAPS: &[Map] = &[\n");
    for (_, id, title) in &listed {
        assert!(!id.is_empty() && id.len() <= 24 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "maps/: {id} is not a map's id");
        assert!(!title.contains('\\'), "maps/: a title is plain text");
        writeln!(out, "    Map {{ id: {id:?}, title: {title:?} }},").unwrap();
    }
    out.push_str("];\n");
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("maps.rs"), out).unwrap();
}
