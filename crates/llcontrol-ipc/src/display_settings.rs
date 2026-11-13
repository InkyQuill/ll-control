//! Global presentation preferences; physical sensor and control units stay canonical.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayTemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}
/// Explicit UI resource language. Older records keep the existing Russian UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceLanguage {
    #[serde(rename = "ru-RU")]
    Russian,
    #[default]
    #[serde(rename = "en-US")]
    English,
}
// v1 omitted Russian on serialization, even for an explicit choice. Missing
// language in an existing record/request must retain that meaning. A new store
// uses DisplaySettings::default instead and starts in English.
fn legacy_language() -> InterfaceLanguage {
    InterfaceLanguage::Russian
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplaySettings {
    pub version: u32,
    pub revision: u64,
    pub temperature_unit: DisplayTemperatureUnit,
    #[serde(default = "legacy_language")]
    pub interface_language: InterfaceLanguage,
}
impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            temperature_unit: DisplayTemperatureUnit::Celsius,
            interface_language: InterfaceLanguage::English,
        }
    }
}
impl DisplaySettings {
    pub fn validate(&self) -> crate::Result<()> {
        if self.version != 1 || self.revision > 9_007_199_254_740_990 {
            return Err(crate::IpcError::InvalidArgument(
                "unsupported display settings version or revision".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_defaults_are_english_but_legacy_records_remain_russian_and_new_writes_are_explicit() {
        assert_eq!(
            DisplaySettings::default().interface_language,
            InterfaceLanguage::English
        );
        assert_eq!(InterfaceLanguage::default(), InterfaceLanguage::English);
        let legacy: DisplaySettings = serde_json::from_value(
            serde_json::json!({"version":1,"revision":7,"temperature_unit":"fahrenheit"}),
        )
        .unwrap();
        assert_eq!(legacy.interface_language, InterfaceLanguage::Russian);
        let json = serde_json::to_value(&legacy).unwrap();
        assert_eq!(json["interface_language"], "ru-RU");
        assert_eq!(json["revision"], 7);
        assert_eq!(json["temperature_unit"], "fahrenheit");
        assert_eq!(
            serde_json::from_value::<DisplaySettings>(json).unwrap(),
            legacy
        );
        assert_eq!(
            serde_json::to_value(DisplaySettings::default()).unwrap()["interface_language"],
            "en-US"
        );
    }
}
