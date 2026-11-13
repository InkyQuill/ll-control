//! Owned explicit Linux sensor/font/resource binding fixture.
use serde_json::{json, Value};
pub fn bindings() -> Value {
    let mut sensors = serde_json::Map::new();
    for (role, unit) in [
        ("CPULoad", "percent"),
        ("GPULoad", "percent"),
        ("CPUTemperature", "celsius"),
        ("GPUTemperature", "celsius"),
        ("CoolantTemp", "celsius"),
        ("PumpRPM", "rpm"),
    ] {
        sensors.insert(role.into(),json!({"sensor_id":format!("explicit:{role}"),"label":format!("Owned {role}"),"unit":unit,"temperature_unit":"fahrenheit","decimals":if unit=="rpm"{0}else{1}}));
    }
    json!({"name":"Owned runtime proposal","sensors":sensors,
        "fonts":{"HyperspaceRace_BoldItalic":{"font":"sans","font_asset":null},"NotoSansTC_Bold":{"font":"noto_sans_tc_bold","font_asset":null}},
        "resources":{"file:///C:/owned/art.png":{"kind":"image","asset":"11111111111111111111111111111111"},"file:///C:/owned/clip.mp4":{"kind":"video","key":"22222222222222222222222222222222"},"C:\\owned\\logo.png":{"kind":"image","asset":"33333333333333333333333333333333"}}})
}
