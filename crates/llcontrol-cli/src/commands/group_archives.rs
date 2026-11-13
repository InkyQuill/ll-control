//! Full-group backup commands; named-slot archive commands remain separate.
use crate::connection::{connect, resolve_device};
use clap::{Subcommand, ValueEnum};
use llcontrol_ipc::{
    client::IpcClient, DeviceId, FullProfileGroup, GroupArchivePhase as Phase,
    GroupArchiveReply as Reply, GroupArchiveRequest as Request, GroupArchiveState,
    MAX_PROFILE_ARCHIVE_BYTES as MAX, PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
};
use serde_json::json;
use std::{io::Write, path::PathBuf, time::Duration};
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Group {
    Screen,
    FanAndLighting,
}
impl From<Group> for FullProfileGroup {
    fn from(group: Group) -> Self {
        match group {
            Group::Screen => Self::Screen,
            Group::FanAndLighting => Self::FanAndLighting,
        }
    }
}
#[derive(Subcommand)]
pub enum Command {
    /// Restore the full FanAndLighting group to LCDV2 Windows2.1.28 defaults.
    ResetFanAndLighting {
        /// Explicit stable CPU temperature sensor identity.
        #[arg(long)]
        sensor: String,
        /// ID for durable readback; retry also requires the original --expected.
        #[arg(long)]
        id: Option<String>,
        /// Target token from group snapshot; otherwise take a fresh snapshot.
        #[arg(long)]
        expected: Option<String>,
        #[arg(long)]
        device: Option<String>,
    },
    /// Read the full source-group snapshot/revision without applying anything.
    Snapshot {
        group: Group,
        #[arg(long)]
        device: Option<String>,
    },
    /// Back up a complete group/all slots/assets to a new LCGROUP1 file.
    Export {
        group: Group,
        file: PathBuf,
        #[arg(long)]
        device: Option<String>,
    },
    /// Validate a full-group backup and report its contents without Apply.
    Inspect {
        group: Group,
        file: PathBuf,
        #[arg(long)]
        device: Option<String>,
    },
    /// Restore the complete group, then report storage and runtime separately.
    Import {
        group: Group,
        file: PathBuf,
        #[arg(long)]
        device: Option<String>,
    },
    /// Recover a durable import result by its operation id, including after restart.
    Readback {
        group: Group,
        id: String,
        #[arg(long)]
        device: Option<String>,
    },
    /// Explicitly repeat Apply of a current imported group, without importing again.
    Reapply {
        group: Group,
        id: String,
        /// Opaque runtime token from readback; otherwise read a fresh one.
        #[arg(long)]
        expected: Option<String>,
        #[arg(long)]
        device: Option<String>,
    },
    Status {
        group: Group,
        id: String,
        #[arg(long)]
        device: Option<String>,
    },
    Cancel {
        group: Group,
        id: String,
        #[arg(long)]
        device: Option<String>,
    },
    Release {
        group: Group,
        id: String,
        #[arg(long)]
        device: Option<String>,
    },
}
async fn rpc(
    client: &IpcClient,
    dev: &DeviceId,
    group: FullProfileGroup,
    request: Request,
) -> anyhow::Result<Reply> {
    Ok(serde_json::from_value(
        client
            .call(
                "group_archive",
                json!({"dev":dev,"group":group,"request":request}),
            )
            .await?,
    )?)
}
fn state(reply: Reply) -> anyhow::Result<GroupArchiveState> {
    match reply {
        Reply::Transfer { state, .. } => Ok(state),
        _ => anyhow::bail!("unexpected group archive response"),
    }
}
pub async fn run(command: Command, as_json: bool) -> anyhow::Result<()> {
    if let Command::ResetFanAndLighting {
        sensor,
        id,
        expected,
        device,
    } = command
    {
        let client = connect().await?;
        let dev = resolve_device(&client, device).await?;
        let id = id.unwrap_or_else(|| format!("{:032x}", rand::random::<u128>()));
        // Print before admission so an interrupted CLI retains a recovery handle.
        eprintln!("FanAndLighting reset operation: {id}");
        let expected = match expected {
            Some(token) => token,
            None => match rpc(
                &client,
                &dev,
                FullProfileGroup::FanAndLighting,
                Request::Snapshot,
            )
            .await?
            {
                Reply::Snapshot { snapshot } => snapshot.expected,
                _ => anyhow::bail!("unexpected group snapshot reply"),
            },
        };
        eprintln!("FanAndLighting reset target token: {expected}");
        let result: llcontrol_ipc::FullGroupCommit = serde_json::from_value(
            client
                .call(
                    "reset_fan_and_lighting",
                    json!({"dev":dev,"operation_id":id,"expected":expected,"sensor_id":sensor}),
                )
                .await?,
        )?;
        anyhow::ensure!(
            result.readback.dev == dev
                && result.readback.group == FullProfileGroup::FanAndLighting
                && result.readback.operation_id == id,
            "reset target mismatch"
        );
        if as_json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!(
                "FanAndLighting defaults committed; repeated: {}\nReadback: {}",
                result.repeated, result.readback.operation_id
            );
        }
        if let Some(error) = result.readback.runtime.last_error {
            anyhow::bail!("reset saved; runtime Apply failed: {error}");
        }
        return Ok(());
    }
    if let Command::Import {
        group,
        file,
        device,
    } = command
    {
        return upload_group(group.into(), file, device, as_json, true).await;
    }
    if let Command::Reapply {
        group,
        id,
        expected,
        device,
    } = command
    {
        let client = connect().await?;
        let dev = resolve_device(&client, device).await?;
        let group = FullProfileGroup::from(group);
        let expected = match expected {
            Some(token) => token,
            None => {
                let current: llcontrol_ipc::FullGroupReadback = serde_json::from_value(
                    client
                        .call(
                            "read_group_import",
                            json!({"dev":dev,"group":group,"operation_id":id}),
                        )
                        .await?,
                )?;
                anyhow::ensure!(
                    current.dev == dev && current.group == group && current.operation_id == id,
                    "readback target mismatch"
                );
                anyhow::ensure!(
                    current.selected_content_current,
                    "imported group is no longer current"
                );
                current.runtime.revision.ok_or_else(|| {
                    anyhow::anyhow!("runtime revision unavailable; read current state")
                })?
            }
        };
        let result: llcontrol_ipc::FullGroupReadback = serde_json::from_value(
            client
                .call(
                    "reapply_group_import",
                    json!({"dev":dev,"group":group,"operation_id":id,"expected":expected}),
                )
                .await?,
        )?;
        anyhow::ensure!(
            result.dev == dev && result.group == group && result.operation_id == id,
            "reapply target mismatch"
        );
        if as_json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("Reapplied {group:?}, operation {id}. Profile was not imported again.");
        }
        if let Some(error) = result.runtime.last_error {
            anyhow::bail!("desired group retained; Apply requires attention: {error}");
        }
        return Ok(());
    }
    if let Command::Readback { group, id, device } = command {
        let client = connect().await?;
        let dev = resolve_device(&client, device).await?;
        let result: llcontrol_ipc::FullGroupReadback = serde_json::from_value(
            client
                .call(
                    "read_group_import",
                    json!({"dev":dev,"group":FullProfileGroup::from(group),"operation_id":id}),
                )
                .await?,
        )?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if let Command::Inspect {
        group,
        file,
        device,
    } = command
    {
        return upload_group(group.into(), file, device, as_json, false).await;
    }
    if let Command::Export {
        group,
        file,
        device,
    } = command
    {
        return export(group.into(), file, device, as_json).await;
    }
    let (group, device, request) = match command {
        Command::Snapshot { group, device } => (group, device, Request::Snapshot),
        Command::Status { group, id, device } => (group, device, Request::Status { id }),
        Command::Cancel { group, id, device } => (group, device, Request::Cancel { id }),
        Command::Release { group, id, device } => (group, device, Request::Release { id }),
        Command::Export { .. }
        | Command::Inspect { .. }
        | Command::Import { .. }
        | Command::Readback { .. }
        | Command::Reapply { .. }
        | Command::ResetFanAndLighting { .. } => unreachable!(),
    };
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let reply = rpc(&client, &dev, group.into(), request).await?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&reply)?);
    } else {
        match &reply {
            Reply::Snapshot { snapshot } => println!(
                "{:?} · {} · {} saved slots\nRevision: {}",
                snapshot.group,
                snapshot.dev.as_str(),
                snapshot.summary.slots.len(),
                snapshot.expected
            ),
            Reply::Transfer { state, .. } => println!(
                "Group archive {}: {:?}; {} bytes produced",
                state.id, state.phase, state.produced
            ),
        }
    }
    if let Reply::Transfer { state, .. } = reply {
        if let Some(error) = state.last_error {
            anyhow::bail!("{error}");
        }
    }
    Ok(())
}
async fn export(
    group: FullProfileGroup,
    path: PathBuf,
    device: Option<String>,
    as_json: bool,
) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => anyhow::bail!("destination already exists"),
        Err(e) => return Err(e.into()),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut output = tempfile::NamedTempFile::new_in(parent)?;
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let Reply::Snapshot { snapshot } = rpc(&client, &dev, group, Request::Snapshot).await? else {
        anyhow::bail!("unexpected group snapshot response")
    };
    anyhow::ensure!(
        snapshot.dev == dev && snapshot.group == group,
        "snapshot target mismatch"
    );
    let id = format!("{:032x}", rand::random::<u128>());
    eprintln!("Group archive {id} · {} · {group:?}", dev.as_str());
    let transfer = async {
        let mut current = state(
            rpc(
                &client,
                &dev,
                group,
                Request::BeginExport {
                    id: id.clone(),
                    expected: snapshot.expected.clone(),
                },
            )
            .await?,
        )?;
        loop {
            anyhow::ensure!(
                current.id == id && current.snapshot == snapshot,
                "group export identity mismatch"
            );
            match current.phase {
                Phase::Ready => break,
                Phase::Preparing => {}
                _ => anyhow::bail!(
                    "{}",
                    current
                        .last_error
                        .as_deref()
                        .unwrap_or("group export cancelled or failed")
                ),
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            current = state(rpc(&client, &dev, group, Request::Status { id: id.clone() }).await?)?;
        }
        let size = current
            .size
            .ok_or_else(|| anyhow::anyhow!("missing archive size"))?;
        anyhow::ensure!((44..=MAX).contains(&size), "invalid archive size");
        let mut offset = 0u64;
        let mut prefix: Vec<u8> = Vec::new();
        loop {
            let Reply::Transfer {
                state,
                offset: position,
                bytes,
                eof,
            } = rpc(
                &client,
                &dev,
                group,
                Request::Read {
                    id: id.clone(),
                    offset,
                    length: CHUNK,
                },
            )
            .await?
            else {
                anyhow::bail!("unexpected group read response")
            };
            anyhow::ensure!(
                state.id == id
                    && state.snapshot == snapshot
                    && state.phase == Phase::Ready
                    && state.size == Some(size)
                    && position == Some(offset),
                "group read acknowledgement mismatch"
            );
            anyhow::ensure!(
                bytes.len() <= CHUNK
                    && offset + bytes.len() as u64 <= size
                    && (!bytes.is_empty() || eof),
                "invalid group chunk"
            );
            anyhow::ensure!(
                eof == (offset + bytes.len() as u64 == size),
                "group EOF mismatch"
            );
            prefix.extend(bytes.iter().take(8usize.saturating_sub(prefix.len())));
            output.write_all(&bytes)?;
            offset += bytes.len() as u64;
            if offset.is_multiple_of(1024 * 1024) || eof {
                eprintln!("Downloaded {offset}/{size} bytes");
            }
            if eof {
                anyhow::ensure!(prefix == b"LCGROUP1", "unexpected full-group archive magic");
                break;
            }
        }
        output.as_file().sync_all()?;
        Ok::<_, anyhow::Error>(current)
    };
    let result = tokio::select! {result=transfer=>result,_=tokio::signal::ctrl_c()=>Err(anyhow::anyhow!("group export interrupted"))};
    if result.is_err() {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            rpc(&client, &dev, group, Request::Cancel { id: id.clone() }),
        )
        .await;
    }
    let _ = tokio::time::timeout(
        Duration::from_secs(2),
        rpc(&client, &dev, group, Request::Release { id }),
    )
    .await;
    let state = result?;
    output.persist_noclobber(&path).map_err(|e| e.error)?;
    std::fs::File::open(parent)?.sync_all()?;
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"file":path,"state":state}))?
        );
    } else {
        println!(
            "Saved {} ({} bytes)",
            path.display(),
            state.size.unwrap_or(0)
        );
    }
    Ok(())
}

async fn upload_rpc(
    client: &IpcClient,
    dev: &DeviceId,
    group: FullProfileGroup,
    request: llcontrol_ipc::GroupUploadRequest,
) -> anyhow::Result<llcontrol_ipc::GroupUploadState> {
    Ok(serde_json::from_value(
        client
            .call(
                "group_upload",
                json!({"dev":dev,"group":group,"request":request}),
            )
            .await?,
    )?)
}
async fn upload_group(
    group: FullProfileGroup,
    path: PathBuf,
    device: Option<String>,
    as_json: bool,
    commit: bool,
) -> anyhow::Result<()> {
    use llcontrol_ipc::{GroupUploadPhase as P, GroupUploadRequest as U};
    use std::io::{Read, Seek};
    let mut file = std::fs::File::open(&path)?;
    anyhow::ensure!(file.metadata()?.is_file(), "backup must be a regular file");
    let size = file.metadata()?.len();
    anyhow::ensure!((44..=MAX).contains(&size), "invalid group backup size");
    let mut magic = [0; 8];
    file.read_exact(&mut magic)?;
    anyhow::ensure!(&magic == b"LCGROUP1", "expected LCGROUP1 full-group backup");
    file.rewind()?;
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let Reply::Snapshot { snapshot } = rpc(&client, &dev, group, Request::Snapshot).await? else {
        anyhow::bail!("unexpected target snapshot")
    };
    anyhow::ensure!(
        snapshot.dev == dev && snapshot.group == group,
        "snapshot target mismatch"
    );
    let id = format!("{:032x}", rand::random::<u128>());
    eprintln!("Group upload {id} · {} · {group:?}", dev.as_str());
    let transfer = async {
        let validate = |s: &llcontrol_ipc::GroupUploadState| -> anyhow::Result<()> {
            anyhow::ensure!(
                s.id == id && s.target == snapshot && s.size == size,
                "group upload identity mismatch"
            );
            anyhow::ensure!(
                s.last_error.is_none(),
                "{}",
                s.last_error.as_deref().unwrap_or("upload failed")
            );
            Ok(())
        };
        let initial = upload_rpc(
            &client,
            &dev,
            group,
            U::Begin {
                id: id.clone(),
                size,
                expected: snapshot.expected.clone(),
            },
        )
        .await?;
        validate(&initial)?;
        anyhow::ensure!(
            initial.phase == P::Uploading && initial.received == 0,
            "unexpected new upload state"
        );
        let mut offset = 0;
        let mut buffer = vec![0; CHUNK];
        while offset < size {
            let length = usize::min(CHUNK, (size - offset) as usize);
            file.read_exact(&mut buffer[..length])?;
            let state = upload_rpc(
                &client,
                &dev,
                group,
                U::Chunk {
                    id: id.clone(),
                    offset,
                    bytes: buffer[..length].to_vec(),
                },
            )
            .await?;
            validate(&state)?;
            offset += length as u64;
            anyhow::ensure!(
                state.phase == P::Uploading && state.received == offset,
                "group chunk acknowledgement mismatch"
            );
        }
        let mut state = upload_rpc(&client, &dev, group, U::Inspect { id: id.clone() }).await?;
        loop {
            validate(&state)?;
            anyhow::ensure!(state.received == size, "group upload incomplete");
            match state.phase {
                P::Prepared => {
                    anyhow::ensure!(
                        state.summary.is_some()
                            && state.source_sha256.as_ref().is_some_and(|s| s.len() == 64),
                        "missing validated group contents"
                    );
                    break;
                }
                P::Inspecting => {}
                _ => anyhow::bail!("group validation cancelled or failed"),
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            state = upload_rpc(&client, &dev, group, U::Status { id: id.clone() }).await?;
        }
        if commit {
            eprintln!("Requesting Finish; on interruption read back operation {id}");
            state = upload_rpc(&client, &dev, group, U::Finish { id: id.clone() }).await?;
            loop {
                validate(&state)?;
                match state.phase {
                    P::Imported => {
                        let result = state
                            .committed
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("missing durable import result"))?;
                        anyhow::ensure!(
                            result.readback.dev == dev
                                && result.readback.group == group
                                && result.readback.operation_id == id,
                            "import result target mismatch"
                        );
                        break;
                    }
                    P::Importing => {}
                    _ => anyhow::bail!("Finish outcome requires readback of operation {id}"),
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
                state = upload_rpc(&client, &dev, group, U::Status { id: id.clone() }).await?;
            }
        }
        Ok::<_, anyhow::Error>(state)
    };
    let result = tokio::select! {result=transfer=>result,_=tokio::signal::ctrl_c()=>Err(if commit {anyhow::anyhow!("group import interrupted; accepted Finish continues; read back operation {id}")}else{anyhow::anyhow!("group inspection interrupted")})};
    // Cancel applies only before Finish admission. An accepted Finish keeps
    // running through commit/Apply; readback is durable after Release/restart.
    let _ = tokio::time::timeout(
        Duration::from_secs(2),
        upload_rpc(&client, &dev, group, U::Cancel { id: id.clone() }),
    )
    .await;
    let _ = tokio::time::timeout(
        Duration::from_secs(2),
        upload_rpc(&client, &dev, group, U::Release { id: id.clone() }),
    )
    .await;
    let state = result?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&state)?);
    } else if commit {
        let result = state
            .committed
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing import acknowledgement"))?;
        println!(
            "Imported {:?}, operation {}. Current content: {}",
            group, id, result.readback.selected_content_current
        );
    } else {
        println!(
            "Validated {:?}: {} saved slots. Settings unchanged.",
            group,
            state.summary.as_ref().map_or(0, |s| s.slots.len())
        );
    }
    if let Some(result) = &state.committed {
        if let Some(error) = &result.readback.runtime.last_error {
            anyhow::bail!(
                "group committed; runtime requires attention: {error}; read back operation {id}"
            );
        }
    }
    Ok(())
}
