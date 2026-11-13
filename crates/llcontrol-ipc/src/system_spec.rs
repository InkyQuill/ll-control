//! Read-only static hardware inventory, independent of numerical sensor samples.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    Cpu,
    Gpu,
    Memory,
    Monitors,
    Motherboard,
    Storage,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SpecValue {
    Text(String),
    Count(u64),
    Bytes(u64),
    Hertz(u64),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecField {
    pub key: String,
    pub value: Option<SpecValue>,
    pub source: String,
    pub unavailable_reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecItem {
    pub id: String,
    pub title: String,
    pub fields: Vec<SpecField>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecGroup {
    pub kind: SpecKind,
    pub items: Vec<SpecItem>,
    pub unavailable_reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemSpec {
    pub version: u32,
    pub collected_at_unix_ms: u64,
    pub groups: Vec<SpecGroup>,
}
impl SystemSpec {
    pub fn validate(&self) -> crate::Result<()> {
        let text = |s: &str| !s.is_empty() && s.len() <= 2048 && !s.chars().any(char::is_control);
        let valid = self.version == 1
            && self.groups.len() == 6
            && [
                SpecKind::Cpu,
                SpecKind::Gpu,
                SpecKind::Memory,
                SpecKind::Monitors,
                SpecKind::Motherboard,
                SpecKind::Storage,
            ]
            .iter()
            .all(|k| self.groups.iter().filter(|g| g.kind == *k).count() == 1)
            && self.groups.iter().all(|g| {
                g.items.len() <= 128
                    && (g.items.is_empty() == g.unavailable_reason.is_some())
                    && g.unavailable_reason.as_deref().is_none_or(text)
                    && g.items.iter().enumerate().all(|(i, item)| {
                        text(&item.id)
                            && text(&item.title)
                            && !g.items[..i].iter().any(|old| old.id == item.id)
                            && !item.fields.is_empty()
                            && item.fields.len() <= 32
                            && item.fields.iter().enumerate().all(|(j, f)| {
                                text(&f.key)
                                    && text(&f.source)
                                    && !item.fields[..j].iter().any(|old| old.key == f.key)
                                    && (f.value.is_none() == f.unavailable_reason.is_some())
                                    && f.unavailable_reason.as_deref().is_none_or(text)
                                    && match &f.value {
                                        Some(SpecValue::Text(s)) => text(s),
                                        _ => true,
                                    }
                            })
                    })
            });
        // Keep the static report comfortably below the shared IPC frame limit.
        let bounded = valid && serde_json::to_vec(self).is_ok_and(|v| v.len() <= 512 * 1024);
        if !bounded {
            return Err(crate::IpcError::InvalidArgument(
                "invalid or oversized system specification".into(),
            ));
        }
        Ok(())
    }
}
