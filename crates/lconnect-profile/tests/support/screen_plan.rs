//! Owned complete source for resource-plan integration; no vendor assets.
#[path = "screen_bindings.rs"]
mod binding;
#[path = "screen_editor.rs"]
mod editor;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
pub const PATH: &str = r"C:\owned\art.png";
pub const URI: &str = "file:///C:/owned/art.png";
pub fn bindings() -> Value {
    let mut value = binding::bindings();
    value["resources"] = json!({});
    value
}
pub fn mapping() -> Value {
    json!({PATH:{"kind":"archive","entry":"wmsq/art.png"},URI:{"kind":"archive","entry":"wmsq/art.png"}})
}
pub fn archive(change: impl FnOnce(&mut Value), extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut input =
        ZipArchive::new(Cursor::new(include_bytes!("../fixtures/ScreenBanks.zip"))).unwrap();
    let mut bytes = Vec::new();
    input
        .by_name("backup")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut backup: Value = serde_json::from_slice(&bytes).unwrap();
    let mut profile: Value = serde_json::from_slice(
        &STANDARD
            .decode(backup["Datas"][0]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    profile["ScreenConfigJson"] = json!(json!({"Style":"Full","Screens":[{"Theme":{"Type":"Image","Path":URI,"BackgroundColor":[3,5,7]},"Text":{"Type":"None"},"Watermark":{"HasImage":false,"Image":"","Horizontal":50,"Vertical":50,"Shadow":50,"Opacity":50}}]}).to_string());
    profile["ScreenProfiles"] = Value::Array((0..8).map(|slot| {
        let mut image=editor::editor(true,slot); image["ImageVideoPath"]=json!(PATH);
        json!({"ImageVideo":[image.to_string()],"BackgroundColor":[editor::editor(false,slot).to_string()],"Rain":[""]})
    }).collect());
    for key in [
        "ScreenImageFileInfos",
        "ScreenImageVideoFileInfos",
        "ScreenWatermarkFileInfos",
    ] {
        profile[key] = json!([{"FilePath":PATH,"IsDefaultFile":false}]);
    }
    change(&mut profile);
    backup["Datas"][0]["Data"] = json!(STANDARD.encode(serde_json::to_vec(&profile).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    zip.start_file("wmsq/art.png", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(include_bytes!("../fixtures/ScreenImage.png"))
        .unwrap();
    for (name, bytes) in extra {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
