//! Owned emulation of public WPF Color properties, not an actual WPF export.
use serde_json::{json, Value};
fn color(rgb: &Value) -> Value {
    let bytes = rgb.as_array().unwrap();
    let linear = |n: u64| {
        let n = n as f64 / 255.0;
        let value = if n <= 0.04045 {
            n / 12.92
        } else {
            ((n + 0.055) / 1.055).powf(2.4)
        };
        f64::from(value as f32)
    };
    json!({"ColorContext":null,"A":255,"R":bytes[0],"G":bytes[1],"B":bytes[2],"ScA":1.0,"ScR":linear(bytes[0].as_u64().unwrap()),"ScG":linear(bytes[1].as_u64().unwrap()),"ScB":linear(bytes[2].as_u64().unwrap())})
}
pub fn storage_colors(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(key.as_str(), "Colors" | "LabelColor") {
                    for rgb in value.as_array_mut().unwrap() {
                        *rgb = color(rgb);
                    }
                } else if key == "BackgroundColor" && value.is_array() {
                    *value = color(value);
                } else {
                    storage_colors(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                storage_colors(value);
            }
        }
        _ => {}
    }
}
