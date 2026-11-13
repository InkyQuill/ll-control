// Combine the independently generated, owned .NET DTO fixtures. This is not a
// genuine vendor export; no vendor assets/assemblies/device identity are used.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
fn backup(bytes: &[u8]) -> Value {
    let mut z = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut data = Vec::new();
    z.by_name("backup").unwrap().read_to_end(&mut data).unwrap();
    serde_json::from_slice(&data).unwrap()
}
pub fn archive() -> Vec<u8> {
    let mut cooling = backup(include_bytes!("../fixtures/Cooling.zip"));
    let lighting = backup(include_bytes!("../fixtures/Lighting.zip"));
    let mut source: Value = serde_json::from_slice(
        &STANDARD
            .decode(cooling["Datas"][1]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let light: Value = serde_json::from_slice(
        &STANDARD
            .decode(lighting["Datas"][0]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    source
        .as_object_mut()
        .unwrap()
        .extend(light.as_object().unwrap().clone());
    cooling["Datas"][1]["Data"] = json!(STANDARD.encode(serde_json::to_vec(&source).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&cooling).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
pub fn bindings() -> Value {
    let mut map = serde_json::Map::new();
    for (role, unit) in [
        ("CPULoad", "percent"),
        ("CPUTemperature", "celsius"),
        ("GPULoad", "percent"),
        ("GPUTemperature", "celsius"),
        ("PumpRPM", "rpm"),
        ("CoolantTemp", "celsius"),
    ] {
        map.insert(
            role.into(),
            json!({"sensor_id":format!("fixture:{role}"),"unit":unit}),
        );
    }
    Value::Object(map)
}
