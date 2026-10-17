//! Owned PNG and synthetic archive payloads; not vendor media.
use serde_json::Value;
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
pub fn archive(image: &[u8], extra_name: &str, extra: &[u8]) -> Vec<u8> {
    let mut input =
        ZipArchive::new(Cursor::new(include_bytes!("../fixtures/ScreenBanks.zip"))).unwrap();
    let mut backup = Vec::new();
    input
        .by_name("backup")
        .unwrap()
        .read_to_end(&mut backup)
        .unwrap();
    let _: Value = serde_json::from_slice(&backup).unwrap();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("backup", backup.as_slice()),
        ("wmsq/art.png", image),
        (extra_name, extra),
    ] {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
