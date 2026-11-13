//! Default WPF Color object serialization is distinct from runtime RGB-array converters.
use crate::Error;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct WpfColor {
    color_context: Value,
    a: u8,
    r: u8,
    g: u8,
    b: u8,
    sc_a: f64,
    sc_r: f64,
    sc_g: f64,
    sc_b: f64,
}
/// Normalize the explicit byte getters consumed by Windows' runtime converter.
/// Keep the original JSON property values as source history. ICC profiles are not loaded.
pub(crate) fn normalize(value: &mut Value) -> Result<BTreeMap<String, Value>, Error> {
    let mut original = BTreeMap::new();
    visit(value, "", &mut original)?;
    Ok(original)
}
fn visit(
    value: &mut Value,
    path: &str,
    original: &mut BTreeMap<String, Value>,
) -> Result<(), Error> {
    match value {
        Value::Object(object) if object.contains_key("R") || object.contains_key("ScR") => {
            let color: WpfColor = serde_json::from_value(value.clone())?;
            if !color.color_context.is_null()
                || ![color.sc_a, color.sc_r, color.sc_g, color.sc_b]
                    .into_iter()
                    .all(f64::is_finite)
            {
                return Err(Error::Invalid(
                    "unsupported WPF color context or nonfinite linear channels",
                ));
            }
            // A/ScA are historical here: the observed runtime converter writes only R/G/B.
            let _alpha = color.a;
            original.insert(path.to_owned(), value.clone());
            *value = serde_json::json!([color.r, color.g, color.b]);
        }
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                let key = key.replace('~', "~0").replace('/', "~1");
                visit(value, &format!("{path}/{key}"), original)?;
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                visit(value, &format!("{path}/{index}"), original)?;
            }
        }
        _ => {}
    }
    Ok(())
}
