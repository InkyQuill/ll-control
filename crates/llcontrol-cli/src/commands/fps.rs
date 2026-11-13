//! Explicit durable FPS source intent; never launches game logging.
use anyhow::{Context, Result};
use clap::{ArgAction, Subcommand};
use llcontrol_ipc::FpsSourceSettings;
use serde_json::json;
#[derive(Subcommand)]
pub enum Command {
    /// Read saved FPS source intent.
    Show,
    /// Replace intent using the revision returned by show. Save is explicit.
    Set {
        #[arg(long)]
        revision: u64,
        #[arg(long,action=ArgAction::Set,required=true)]
        enabled: bool,
        #[arg(long)]
        log_path: Option<String>,
    },
}
pub async fn run(command: Command, json_mode: bool) -> Result<()> {
    let client = crate::connection::connect().await?;
    let value = match command {
        Command::Show => client.call("get_fps_source", json!({})).await?,
        Command::Set {
            revision,
            enabled,
            log_path,
        } => {
            let settings = FpsSourceSettings {
                version: 1,
                revision,
                enabled,
                log_path,
            };
            settings.validate()?;
            client
                .call("set_fps_source", json!({"settings":settings}))
                .await?
        }
    };
    let settings: FpsSourceSettings =
        serde_json::from_value(value).context("invalid FPS settings response")?;
    settings.validate()?;
    if json_mode {
        println!("{}", serde_json::to_string(&settings)?);
    } else {
        println!(
            "FPS source: {}",
            if settings.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("Revision: {}", settings.revision);
        println!(
            "MangoHud per-frame log: {}",
            settings.log_path.as_deref().unwrap_or("not selected")
        );
    }
    Ok(())
}
