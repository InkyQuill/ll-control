//! Explicit target-bound Windows FanAndLighting preview/import.
use crate::connection::{connect, resolve_device};
use clap::Args;
use serde_json::json;
use std::{io::Read, path::PathBuf};
#[derive(Args)]
pub struct Options {
    pub file: PathBuf,
    #[arg(long)]
    pub record: usize,
    #[arg(long)]
    pub bindings: PathBuf,
    #[arg(long, default_value_t = 0.0)]
    pub hysteresis_c: f64,
    #[arg(long, default_value_t = 100)]
    pub fallback_percent: u8,
    /// Commit and Apply explicitly; omitted means read-only preview.
    #[arg(long)]
    pub apply: bool,
    #[arg(long)]
    pub device: Option<String>,
    /// Retry with the original operation ID and target token, never a fresh token.
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long)]
    pub expected: Option<String>,
}
pub async fn run(options: Options, as_json: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        options.id.is_some() == options.expected.is_some(),
        "retry requires both --id and original --expected"
    );
    anyhow::ensure!(
        options.apply || options.id.is_none(),
        "retry options require explicit --apply"
    );
    let file = std::fs::File::open(options.bindings)?;
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    let bindings = llcontrol_profile::linux_lighting::parse_bindings(&bytes)?;
    let draft = llcontrol_profile::control::prepare(
        std::fs::File::open(options.file)?,
        options.record,
        &bindings,
        options.hysteresis_c,
        options.fallback_percent,
    )?;
    let draft_json = serde_json::to_string(&draft)?;
    // Check the exact bounded admission representation before opening IPC.
    llcontrol_profile::control::parse(draft_json.as_bytes())?;
    let client = connect().await?;
    let dev = resolve_device(&client, options.device).await?;
    if !options.apply {
        let preview: llcontrol_ipc::WindowsControlPreview = serde_json::from_value(
            client
                .call(
                    "preview_windows_control_import",
                    json!({"dev":dev,"draft_json":draft_json}),
                )
                .await?,
        )?;
        anyhow::ensure!(
            preview.target.dev == dev
                && preview.target.group == llcontrol_ipc::FullProfileGroup::FanAndLighting,
            "Windows import preview target mismatch"
        );
        println!("{}", serde_json::to_string_pretty(&preview)?);
        return Ok(());
    }
    let id = options
        .id
        .unwrap_or_else(|| format!("{:032x}", rand::random::<u128>()));
    let expected = match options.expected {
        Some(token) => token,
        None => {
            let preview: llcontrol_ipc::WindowsControlPreview = serde_json::from_value(
                client
                    .call(
                        "preview_windows_control_import",
                        json!({"dev":dev,"draft_json":draft_json}),
                    )
                    .await?,
            )?;
            anyhow::ensure!(
                preview.target.dev == dev
                    && preview.target.group == llcontrol_ipc::FullProfileGroup::FanAndLighting,
                "Windows import preview target mismatch"
            );
            preview.target.expected
        }
    };
    eprintln!("Windows FanAndLighting import operation: {id}\nOriginal target token: {expected}");
    let result: llcontrol_ipc::FullGroupCommit = serde_json::from_value(
        client
            .call(
                "import_windows_control",
                json!({"dev":dev,"operation_id":id,"expected":expected,"draft_json":draft_json}),
            )
            .await?,
    )?;
    anyhow::ensure!(
        result.readback.dev == dev
            && result.readback.operation_id == id
            && result.readback.group == llcontrol_ipc::FullProfileGroup::FanAndLighting,
        "Windows import readback target mismatch"
    );
    if as_json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "Windows FanAndLighting group committed; repeated: {}\n{}",
            result.repeated,
            serde_json::to_string_pretty(&result.readback)?
        );
    }
    anyhow::ensure!(
        result.readback.runtime.last_error.is_none()
            && result.readback.runtime.recovery_required != Some(true),
        "group committed but runtime needs recovery; use the printed operation ID for readback"
    );
    Ok(())
}
