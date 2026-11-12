// Owned synthetic values from public DTO field shapes; no vendor source/assets.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, ZipWriter};
const MODES: &[&str] = &[
    "Rainbow",
    "RainbowMorph",
    "StaticColor",
    "Breathing",
    "Runway",
    "Meteor",
    "TickerTape",
    "Fluctuation",
    "Transmit",
    "ColorfulStarryNight",
    "StaticStarryNight",
    "Voice",
    "BigBang",
    "Burst",
    "ColorsMorph",
    "Bounce",
];
pub fn profile() -> Value {
    let mut fan = serde_json::Map::new();
    let mut pump = serde_json::Map::new();
    for (i, name) in MODES.iter().enumerate() {
        let count = [0, 0, 1, 1, 2, 4, 4, 4, 2, 0, 1, 1, 4, 1, 0, 4][i];
        let colors: Vec<_> = (0..count)
            .map(|c| json!([17 + i as u8, c as u8, 99]))
            .collect();
        let p = json!({"Colors":colors,"Speed":if i==2{i32::MIN}else{42},"Brightness":73,"Direction":0});
        if i < 6 {
            let mut f = p.clone();
            f["Mode"] = json!(i + 1);
            fan.insert((*name).into(), f);
        }
        let mut sensors = serde_json::Map::new();
        for (s, sensor) in [
            "CPULoad",
            "CPUTemperature",
            "GPULoad",
            "GPUTemperature",
            "PumpRPM",
            "CoolantTemp",
        ]
        .iter()
        .enumerate()
        {
            let mut high = p.clone();
            high["Brightness"] = json!(90 - s);
            let mut low = p.clone();
            low["Brightness"] = json!(20 + s);
            sensors.insert((*sensor).into(),json!({"High":high,"Low":low,"HighValue":60+s,"LowValue":30+s,"MinValue":0,"MaxValue":3600,"UnitText":if s==4{"R"}else if s.is_multiple_of(2){"%"}else{"°C"}}));
        }
        pump.insert((*name).into(),json!({"Mode":i+1,"IsDynamicMode":i.is_multiple_of(2),"Static":p,"DynamicSettings":sensors,"SensorType":i%6+1}));
    }
    json!({"FanLEDMode":4,"ScreenLEDMode":13,"FanLEDProfiles":fan,"ScreenLEDProfiles":pump,"FanLEDSyncToScreenLED":false,"FanLEDSize":0,"FanLEDLoopNumber":37,"ScreenBrightness":51})
}
pub fn archive(profile: &Value, group: u8) -> Vec<u8> {
    let backup = json!({"Version":"2.1.28.0","Type":group,"Datas":[{"MainType":0,"SubType":16846849,"Metadata":"PRIVATE-WINDOWS-SERIAL","Data":STANDARD.encode(serde_json::to_vec(profile).unwrap())}]});
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
