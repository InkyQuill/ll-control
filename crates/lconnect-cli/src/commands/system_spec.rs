//! Static hardware inventory, independent of attached Lian Li devices.
use anyhow::{Context, Result};
use lconnect_ipc::{SpecValue, SystemSpec};
use serde_json::json;
pub async fn run(json_mode: bool, refresh: bool) -> Result<()> {
    let client = crate::connection::connect().await?;
    let spec: SystemSpec = serde_json::from_value(
        client
            .call("get_system_spec", json!({"refresh":refresh}))
            .await
            .context("get_system_spec RPC failed")?,
    )?;
    spec.validate()?;
    if json_mode {
        println!("{}", serde_json::to_string(&spec)?);
    } else {
        for group in spec.groups {
            println!("\n{:?}", group.kind);
            if let Some(reason) = group.unavailable_reason {
                println!("  Unavailable: {reason}");
            }
            for item in group.items {
                println!("  {}", item.title);
                for f in item.fields {
                    let value = match f.value {
                        Some(SpecValue::Text(v)) => v,
                        Some(SpecValue::Count(v)) => v.to_string(),
                        Some(SpecValue::Bytes(v)) => format!("{v} bytes"),
                        Some(SpecValue::Hertz(v)) => format!("{v} Hz"),
                        None => format!(
                            "Unavailable: {}",
                            f.unavailable_reason.as_deref().unwrap_or("unknown")
                        ),
                    };
                    println!("    {}: {} [{}]", f.key, value, f.source);
                }
            }
        }
    }
    Ok(())
}
