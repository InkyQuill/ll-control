use crate::connection::{connect, resolve_device};
use clap::{Subcommand, ValueEnum};
use lconnect_ipc::{NamedProfile, ProfileContent, ProfileEdit, ProfileGroup, ProfileLibrary};
use serde_json::json;
use std::{io::Read, path::PathBuf};

#[derive(Clone, Copy, ValueEnum)]
pub enum Group {
    Screen,
    Fan,
    Pump,
}
impl From<Group> for ProfileGroup {
    fn from(value: Group) -> Self {
        match value {
            Group::Screen => Self::Screen,
            Group::Fan => Self::Fan,
            Group::Pump => Self::Pump,
        }
    }
}
#[derive(Subcommand)]
pub enum Command {
    /// Full-group snapshots/backups, distinct from individual saved slots.
    Group {
        #[command(subcommand)]
        command: super::group_archives::Command,
    },
    /// Export one slot and all dependencies to a new portable Linux archive.
    Export {
        group: Group,
        slot: u8,
        file: PathBuf,
    },
    /// Import into one slot without applying device settings.
    Import {
        group: Group,
        slot: u8,
        file: PathBuf,
    },
    /// Recover an archive operation by its printed ID, including after restart.
    ArchiveState {
        id: String,
    },
    /// Retry/read back Finish; never upload or apply a profile again.
    ArchiveFinish {
        id: String,
    },
    ArchiveCancel {
        id: String,
    },
    /// List saved slots; this does not need a connected device.
    List,
    /// Save a NamedProfile JSON document without applying it. Slots are zero-based.
    Save {
        file: PathBuf,
    },
    /// Restore one slot to Linux defaults; cooling requires an explicit sensor.
    Reset {
        group: Group,
        slot: u8,
        #[arg(long)]
        sensor: Option<String>,
    },
    /// Restore all slots in a group in one library edit, without applying them.
    /// Does not reset device brightness, rotation, active content, or galleries.
    ResetGroup {
        group: Group,
        #[arg(long)]
        sensor: Option<String>,
    },
    Rename {
        group: Group,
        slot: u8,
        name: String,
    },
    Delete {
        group: Group,
        slot: u8,
    },
    /// Apply the stored snapshot through the existing LCD/cooling owner.
    Apply {
        group: Group,
        slot: u8,
        #[arg(long)]
        device: Option<String>,
    },
}

pub async fn run(command: Command, as_json: bool) -> anyhow::Result<()> {
    let command = match command {
        Command::Group { command } => return super::group_archives::run(command, as_json).await,
        other => other,
    };
    if matches!(
        &command,
        Command::Export { .. }
            | Command::Import { .. }
            | Command::ArchiveState { .. }
            | Command::ArchiveFinish { .. }
            | Command::ArchiveCancel { .. }
    ) {
        return run_archive(command, as_json).await;
    }
    // Reject local input before even attempting IPC.
    let edit = match &command {
        Command::Save { file } => {
            let mut bytes = Vec::new();
            std::fs::File::open(file)?
                .take(65537)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 65536, "profile exceeds 64 KiB");
            let profile: NamedProfile = serde_json::from_slice(&bytes)?;
            Some(ProfileEdit::Save { profile })
        }
        Command::Reset {
            group,
            slot,
            sensor,
        } => Some(ProfileEdit::Reset {
            group: (*group).into(),
            slot: *slot,
            sensor_id: sensor.clone(),
        }),
        Command::ResetGroup { group, sensor } => Some(ProfileEdit::ResetGroup {
            group: (*group).into(),
            sensor_id: sensor.clone(),
        }),
        Command::Rename { group, slot, name } => Some(ProfileEdit::Rename {
            group: (*group).into(),
            slot: *slot,
            name: name.clone(),
        }),
        Command::Delete { group, slot } => Some(ProfileEdit::Delete {
            group: (*group).into(),
            slot: *slot,
        }),
        Command::Apply { group, slot, .. } => {
            ProfileGroup::from(*group).validate_slot(*slot)?;
            None
        }
        Command::List => None,
        Command::Group { .. }
        | Command::Export { .. }
        | Command::Import { .. }
        | Command::ArchiveState { .. }
        | Command::ArchiveFinish { .. }
        | Command::ArchiveCancel { .. } => unreachable!("handled above"),
    };
    if let Some(edit) = &edit {
        edit.validate()?;
    }
    let client = connect().await?;
    let mut library: ProfileLibrary =
        serde_json::from_value(client.call("list_profiles", json!({})).await?)?;
    library.validate()?;
    if let Some(edit) = edit {
        library = serde_json::from_value(
            client
                .call(
                    "edit_profile",
                    json!({"expected_revision":library.revision,"edit":edit}),
                )
                .await?,
        )?;
    }
    if let Command::Apply {
        group,
        slot,
        device,
    } = command
    {
        let group = ProfileGroup::from(group);
        let profile = library
            .profiles
            .iter()
            .find(|p| p.slot == slot && p.content.group() == group)
            .ok_or_else(|| anyhow::anyhow!("profile slot is empty"))?;
        let dev = resolve_device(&client, device).await?;
        let result = match &profile.content {
            ProfileContent::Screen(theme) => {
                client
                    .call("set_lcd_theme", json!({"dev":dev,"theme":theme}))
                    .await?
            }
            ProfileContent::ScreenMedia(key) => {
                client
                    .call("set_lcd_media", json!({"dev":dev,"key":key}))
                    .await?
            }
            ProfileContent::Fan(policy) => {
                client
                    .call(
                        "set_cooling_policy",
                        json!({"dev":dev,"target":"fan","policy":policy}),
                    )
                    .await?
            }
            ProfileContent::Pump(policy) => {
                client
                    .call(
                        "set_cooling_policy",
                        json!({"dev":dev,"target":"pump","policy":policy}),
                    )
                    .await?
            }
        };
        let error = match group {
            ProfileGroup::Screen => result.get("last_error"),
            ProfileGroup::Fan => result.get("fan").and_then(|v| v.get("last_error")),
            ProfileGroup::Pump => result.get("pump").and_then(|v| v.get("last_error")),
        }
        .and_then(serde_json::Value::as_str);
        if let Some(error) = error {
            anyhow::bail!("profile was requested but failed to apply: {error}");
        }
        if as_json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("Applied profile: {}", profile.name);
        }
    } else if as_json {
        println!("{}", serde_json::to_string_pretty(&library)?);
    } else {
        println!("Profiles (revision {})", library.revision);
        for profile in &library.profiles {
            println!(
                "{:?} [{}] {}",
                profile.content.group(),
                profile.slot,
                profile.name
            );
        }
        if library.profiles.is_empty() {
            println!("No saved profiles.");
        }
    }
    Ok(())
}

async fn archive_rpc(
    client: &lconnect_ipc::client::IpcClient,
    request: lconnect_ipc::ProfileArchiveRequest,
) -> anyhow::Result<lconnect_ipc::ProfileArchiveReply> {
    let result = client
        .call("profile_archive", json!({"request":request}))
        .await?;
    Ok(serde_json::from_value(result)?)
}
fn show_archive(reply: &lconnect_ipc::ProfileArchiveReply, as_json: bool) -> anyhow::Result<()> {
    if as_json {
        println!("{}", serde_json::to_string_pretty(reply)?);
    } else {
        println!(
            "Archive {}: {:?}; {}/{} bytes; committed revision {:?}",
            reply.state.id,
            reply.state.phase,
            reply.state.received,
            reply.state.size,
            reply.state.committed_revision
        );
    }
    if let Some(error) = &reply.state.last_error {
        anyhow::bail!("{error}");
    }
    Ok(())
}
async fn run_archive(command: Command, as_json: bool) -> anyhow::Result<()> {
    use lconnect_ipc::{
        ProfileArchivePhase as Phase, ProfileArchiveRequest as Request,
        MAX_PROFILE_ARCHIVE_BYTES as MAX_BYTES, PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
    };
    use std::io::{Seek, SeekFrom, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    let (group, slot, path, import) = match command {
        Command::Export { group, slot, file } => (ProfileGroup::from(group), slot, file, false),
        Command::Import { group, slot, file } => (ProfileGroup::from(group), slot, file, true),
        control => {
            let cancel = matches!(&control, Command::ArchiveCancel { .. });
            let request = match control {
                Command::ArchiveState { id } => Request::Status { id },
                Command::ArchiveFinish { id } => Request::Finish { id },
                Command::ArchiveCancel { id } => Request::Cancel { id },
                _ => unreachable!(),
            };
            let client = connect().await?;
            let reply = archive_rpc(&client, request).await?;
            if cancel {
                if reply.state.phase == Phase::Imported {
                    eprintln!("Import was already committed; cancellation does not undo it.");
                }
                let _ = archive_rpc(
                    &client,
                    Request::Release {
                        id: reply.state.id.clone(),
                    },
                )
                .await;
            }
            return show_archive(&reply, as_json);
        }
    };
    group.validate_slot(slot)?;
    let mut input = if import {
        let mut file = std::fs::File::open(&path)?;
        let metadata = file.metadata()?;
        anyhow::ensure!(
            metadata.is_file() && (44..=MAX_BYTES).contains(&metadata.len()),
            "invalid archive file type/length"
        );
        let mut prefix = [0; 12];
        file.read_exact(&mut prefix)?;
        anyhow::ensure!(
            &prefix[..8] == b"LCPROF01",
            "not a portable Linux profile archive"
        );
        let length = u32::from_le_bytes(prefix[8..].try_into()?);
        anyhow::ensure!(
            (1..=1024 * 1024).contains(&length) && 44 + u64::from(length) <= metadata.len(),
            "invalid archive manifest length"
        );
        file.seek(SeekFrom::Start(0))?;
        Some(file)
    } else {
        None
    };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut output = if !import {
        anyhow::ensure!(
            std::fs::symlink_metadata(&path)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "archive destination already exists or is unreadable"
        );
        Some(tempfile::NamedTempFile::new_in(parent)?)
    } else {
        None
    };
    let client = connect().await?;
    let library: ProfileLibrary =
        serde_json::from_value(client.call("list_profiles", json!({})).await?)?;
    library.validate()?;
    let id = format!("{:032x}", rand::random::<u128>());
    eprintln!(
        "Archive operation: {id} (library revision {})",
        library.revision
    );
    let finishing = AtomicBool::new(false);
    let work = async {
        let result = if let Some(file) = &mut input {
            let size = file.metadata()?.len();
            let state = archive_rpc(
                &client,
                Request::BeginImport {
                    id: id.clone(),
                    size,
                    expected_revision: library.revision,
                    group,
                    slot,
                },
            )
            .await?;
            anyhow::ensure!(
                state.state.id == id && state.state.phase == Phase::Uploading,
                "unexpected import session"
            );
            let mut offset = 0;
            let mut buffer = vec![0; CHUNK];
            while offset < size {
                let count = (size - offset).min(CHUNK as u64) as usize;
                file.read_exact(&mut buffer[..count])?;
                let state = archive_rpc(
                    &client,
                    Request::Chunk {
                        id: id.clone(),
                        offset,
                        bytes: buffer[..count].to_vec(),
                    },
                )
                .await?;
                offset += count as u64;
                anyhow::ensure!(
                    state.state.id == id
                        && state.state.size == size
                        && state.state.received == offset
                        && state.state.phase == Phase::Uploading,
                    "archive upload acknowledgement mismatch"
                );
                if offset % (1024 * 1024) == 0 || offset == size {
                    eprintln!("Uploaded {offset}/{size} bytes");
                }
            }
            let mut tail = [0];
            anyhow::ensure!(
                file.read(&mut tail)? == 0 && file.metadata()?.len() == size,
                "archive changed during upload"
            );
            archive_rpc(&client, Request::Inspect { id: id.clone() }).await?;
            // Cancellation cannot race the atomic profile publication as an undo.
            finishing.store(true, Ordering::Release);
            let done = archive_rpc(&client, Request::Finish { id: id.clone() }).await?;
            anyhow::ensure!(
                done.state.id == id && done.state.phase == Phase::Imported,
                "{}",
                done.state
                    .last_error
                    .as_deref()
                    .unwrap_or("archive was not committed; read back operation state")
            );
            done
        } else {
            let state = archive_rpc(
                &client,
                Request::BeginExport {
                    id: id.clone(),
                    expected_revision: library.revision,
                    group,
                    slot,
                },
            )
            .await?;
            let size = state.state.size;
            anyhow::ensure!(
                state.state.id == id && state.state.phase == Phase::Ready && size <= MAX_BYTES,
                "unexpected export session"
            );
            let mut offset = 0;
            loop {
                let part = archive_rpc(
                    &client,
                    Request::Read {
                        id: id.clone(),
                        offset,
                        length: CHUNK,
                    },
                )
                .await?;
                anyhow::ensure!(
                    part.state.id == id
                        && part.offset == Some(offset)
                        && part.state.size == size
                        && part.bytes.len() <= CHUNK
                        && part.state.phase == Phase::Ready,
                    "archive download metadata mismatch"
                );
                let end = offset
                    .checked_add(part.bytes.len() as u64)
                    .ok_or_else(|| anyhow::anyhow!("archive offset overflow"))?;
                anyhow::ensure!(
                    end <= size
                        && part.eof == (end == size)
                        && (!part.bytes.is_empty() || part.eof),
                    "incomplete archive download"
                );
                output.as_mut().unwrap().write_all(&part.bytes)?;
                offset = end;
                if offset % (1024 * 1024) == 0 || part.eof {
                    eprintln!("Downloaded {offset}/{size} bytes");
                }
                if part.eof {
                    break;
                }
            }
            let file = output.take().unwrap();
            file.as_file().sync_all()?;
            file.persist_noclobber(&path).map_err(|e| e.error)?;
            std::fs::File::open(parent)?.sync_all()?;
            let _ = archive_rpc(&client, Request::Cancel { id: id.clone() }).await;
            state
        };
        // The durable import receipt remains in the profile library after release.
        let _ = archive_rpc(&client, Request::Release { id: id.clone() }).await;
        show_archive(&result, as_json)
    };
    tokio::select! {
        result=work=>{
            if result.is_err()&&!finishing.load(Ordering::Acquire) {let _=archive_rpc(&client,Request::Cancel{id:id.clone()}).await;let _=archive_rpc(&client,Request::Release{id:id.clone()}).await;}
            if result.is_err()&&finishing.load(Ordering::Acquire){eprintln!("Read back with: lconnectctl profiles archive-state {id}");}
            result
        }
        _=tokio::signal::ctrl_c()=>{
            if !finishing.load(Ordering::Acquire){let _=archive_rpc(&client,Request::Cancel{id:id.clone()}).await;let _=archive_rpc(&client,Request::Release{id:id.clone()}).await;}
            anyhow::bail!("archive command interrupted; read back with: lconnectctl profiles archive-state {id}")
        }
    }
}
