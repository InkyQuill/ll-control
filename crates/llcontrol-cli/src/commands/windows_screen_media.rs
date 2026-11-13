//! Offline preparation retains its blocking worker until cancellation has reaped children.
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
enum Work {
    Media,
    Plan { bindings: PathBuf, map: PathBuf },
}
pub async fn run(file: PathBuf, record: usize) -> anyhow::Result<()> {
    run_owned(file, record, Work::Media).await
}
pub async fn plan(
    file: PathBuf,
    record: usize,
    bindings: PathBuf,
    map: PathBuf,
) -> anyhow::Result<()> {
    run_owned(file, record, Work::Plan { bindings, map }).await
}
fn bounded(path: PathBuf) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
async fn run_owned(file: PathBuf, record: usize, work: Work) -> anyhow::Result<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let mut worker = tokio::task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
        match work {
            Work::Media => {
                let (source, media) = llcontrol_profile::screen_media::inspect_and_prepare(
                    std::fs::File::open(file)?,
                    record,
                    &worker_cancel,
                )?;
                Ok(
                    serde_json::json!({"source":source,"prepared_media":media,"preparation_only":true,"apply_supported":false,"group_import_supported":false}),
                )
            }
            Work::Plan { bindings, map } => {
                let bindings =
                    llcontrol_profile::linux_screen::parse_bindings(&bounded(bindings)?)?;
                let map = llcontrol_profile::screen_plan::parse_resource_map(&bounded(map)?)?;
                let (source, plan) = llcontrol_profile::screen_plan::inspect_and_prepare(
                    std::fs::File::open(file)?,
                    record,
                    &bindings,
                    &map,
                    &worker_cancel,
                )?;
                Ok(
                    serde_json::json!({"source":source,"screen_plan":plan,"preparation_only":true,"apply_supported":false,"group_import_supported":false}),
                )
            }
        }
    });
    let report = tokio::select! {
        result=&mut worker=>result??,
        signal=tokio::signal::ctrl_c()=>{cancel.store(true,Ordering::Release);let _=worker.await;signal?;anyhow::bail!("Screen preparation cancelled; worker and media tools joined");}
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
