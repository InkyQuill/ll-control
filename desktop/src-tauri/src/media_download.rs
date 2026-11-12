//! Dedicated-connection gallery source download; no media publication or HID.
use crate::media_prepare::{ExportWriter, PreparedFile};
use anyhow::{bail, ensure, Context, Result};
use lconnect_ipc::{client::IpcClient, MediaExportEvent, MediaExportInfo};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Opened {
    stream_id: String,
    info: MediaExportInfo,
}

/// Takes sole ownership of a dedicated client. Cancellation drops the client,
/// stopping both wire tasks. An in-flight disk write keeps its staging owner
/// until the blocking task exits; no partially downloaded file escapes.
/// Caller supplies a fresh 32-lowercase-hex stream id.
pub async fn download(client: IpcClient, key: String, stream_id: String) -> Result<PreparedFile> {
    let (_cancel, cancellation) = tokio::sync::watch::channel(false);
    download_cancellable(client, key, stream_id, cancellation).await
}

async fn cancelled(cancellation: &mut tokio::sync::watch::Receiver<bool>) {
    if cancellation.wait_for(|value| *value).await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// Cooperative cancellation interrupts network waits, but always joins an
/// admitted blocking write before returning, so a job can safely reopen after it.
pub async fn download_cancellable(
    client: IpcClient,
    key: String,
    stream_id: String,
    mut cancellation: tokio::sync::watch::Receiver<bool>,
) -> Result<PreparedFile> {
    ensure!(
        stream_id.len() == 32
            && stream_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Invalid export stream id"
    );
    let mut events = client.subscribe_reliable("media_export").await;
    let response = tokio::select! {
        biased;
        _ = cancelled(&mut cancellation) => bail!("Media export cancelled"),
        response = async { Ok::<_,anyhow::Error>(tokio::time::timeout(
        Duration::from_secs(10),
        client.call(
            "open_media_export",
            json!({"key":key,"stream_id":stream_id}),
        ),
    )
    .await
    .context("Media export open timed out")??) } => response?,
    };
    let opened: Opened = serde_json::from_value(response)?;
    ensure!(opened.stream_id == stream_id, "Unexpected export stream");
    let mut writer = tokio::task::spawn_blocking(move || ExportWriter::new(opened.info)).await??;
    loop {
        let event = tokio::select! {
            biased;
            _ = cancelled(&mut cancellation) => bail!("Media export cancelled"),
            event = async { tokio::time::timeout(Duration::from_secs(30), events.recv())
            .await
            .context("Media export stalled")?
            .context("Media export disconnected before completion") } => event?,
        };
        match serde_json::from_value::<MediaExportEvent>(event.params)? {
            MediaExportEvent::Unit {
                stream_id: id,
                index,
                unit,
            } => {
                ensure!(id == stream_id, "Unexpected export stream");
                writer = tokio::task::spawn_blocking(move || writer.push(index, unit)).await??;
            }
            MediaExportEvent::Complete { stream_id: id } => {
                ensure!(id == stream_id, "Unexpected export stream");
                let prepared = tokio::task::spawn_blocking(move || writer.finish()).await??;
                ensure!(!*cancellation.borrow(), "Media export cancelled");
                return Ok(prepared);
            }
            MediaExportEvent::Failed {
                stream_id: id,
                message,
            } => {
                ensure!(id == stream_id, "Unexpected export stream");
                bail!("Media export failed: {message}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lconnect_ipc::{jsonrpc::Notification, H264AccessUnit};
    use std::sync::atomic::AtomicBool;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    async fn source(
        events: Vec<serde_json::Value>,
        frames: usize,
        bytes: u64,
    ) -> (IpcClient, tokio::task::JoinHandle<()>) {
        let (a, b) = tokio::io::duplex(1024);
        let server = tokio::spawn(async move {
            let mut peer = BufReader::new(b);
            peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
                .await
                .unwrap();
            let mut request = String::new();
            peer.read_line(&mut request).await.unwrap();
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":{"stream_id":"a".repeat(32),"info":{"fps":24,"frames":frames,"encoded_bytes":bytes}}});
            peer.write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
            for event in events {
                let notification = Notification::new("media_export", event);
                if peer
                    .write_all(
                        format!("{}\n", serde_json::to_string(&notification).unwrap()).as_bytes(),
                    )
                    .await
                    .is_err()
                {
                    return;
                }
            }
            // Wait for download ownership to close the dedicated connection.
            let mut byte = [0];
            let _ = peer.read(&mut byte).await;
        });
        (IpcClient::from_stream(a).await.unwrap(), server)
    }
    fn event(index: usize, unit: H264AccessUnit) -> serde_json::Value {
        json!({"event":"unit","stream_id":"a".repeat(32),"index":index,"unit":unit})
    }
    fn tiny() -> H264AccessUnit {
        H264AccessUnit {
            bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
            keyframe: true,
        }
    }

    #[tokio::test]
    async fn cancellation_interrupts_open_and_event_waits_and_closes_wire() {
        for opened in [false, true] {
            let (a, b) = tokio::io::duplex(1024);
            let mut peer = BufReader::new(b);
            peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
                .await
                .unwrap();
            let client = IpcClient::from_stream(a).await.unwrap();
            let (cancel, signal) = tokio::sync::watch::channel(false);
            let download = tokio::spawn(download_cancellable(
                client,
                "saved".into(),
                "a".repeat(32),
                signal,
            ));
            let mut line = String::new();
            peer.read_line(&mut line).await.unwrap();
            if opened {
                let request: serde_json::Value = serde_json::from_str(&line).unwrap();
                let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":{"stream_id":"a".repeat(32),"info":{"fps":24,"frames":2,"encoded_bytes":30}}});
                peer.write_all(format!("{reply}\n").as_bytes())
                    .await
                    .unwrap();
                let notification = Notification::new("media_export", event(0, tiny()));
                peer.write_all(
                    format!("{}\n", serde_json::to_string(&notification).unwrap()).as_bytes(),
                )
                .await
                .unwrap();
            }
            cancel.send(true).unwrap();
            assert!(tokio::time::timeout(Duration::from_secs(2), download)
                .await
                .unwrap()
                .unwrap()
                .is_err());
            let mut byte = [0];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), peer.read(&mut byte))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        }
    }

    #[tokio::test]
    async fn malformed_or_partial_transfer_never_yields_prepared_media() {
        let complete = json!({"event":"complete","stream_id":"a".repeat(32)});
        for events in [
            vec![complete.clone()],
            vec![event(1, tiny()), complete.clone()],
            vec![event(0, tiny()), event(0, tiny()), complete.clone()],
            vec![
                event(0, tiny()),
                json!({"event":"failed","stream_id":"a".repeat(32),"message":"corrupt"}),
            ],
            vec![json!({"event":"complete","stream_id":"b".repeat(32)})],
            vec![
                event(
                    0,
                    H264AccessUnit {
                        bytes: vec![1],
                        keyframe: true,
                    },
                ),
                complete,
            ],
        ] {
            let (client, server) = source(events, 1, 15).await;
            assert!(download(client, "saved".into(), "a".repeat(32))
                .await
                .is_err());
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .unwrap()
                .unwrap();
        }
    }

    #[tokio::test]
    async fn real_frames_download_remux_and_release_temporary_source() {
        let (units, bytes) = tokio::task::spawn_blocking(|| {
            let root = tempfile::tempdir().unwrap();
            let input = root.path().join("source.mp4");
            assert!(std::process::Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=s=480x480:r=24",
                    "-t",
                    "0.5",
                    "-threads",
                    "1"
                ])
                .arg(&input)
                .status()
                .unwrap()
                .success());
            let mut prepared = crate::media_prepare::prepare_file(
                &input,
                crate::media_prepare::Fit::Contain,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            let units = (0..prepared.frames())
                .map(|i| prepared.read_unit(i).unwrap())
                .collect::<Vec<_>>();
            (units, prepared.encoded_bytes())
        })
        .await
        .unwrap();
        let count = units.len();
        let mut events = units
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, u)| event(i, u))
            .collect::<Vec<_>>();
        events.push(json!({"event":"complete","stream_id":"a".repeat(32)}));
        let (client, server) = source(events, count, bytes).await;
        let mut prepared = download(client, "saved".into(), "a".repeat(32))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        tokio::task::spawn_blocking(move || {
            assert_eq!(prepared.frames(), 12);
            for (index, expected) in units.iter().enumerate() {
                assert_eq!(&prepared.read_unit(index).unwrap(), expected);
            }
            let preview = prepared.preview(&AtomicBool::new(false)).unwrap();
            assert!(preview.length() > 0);
            let path = prepared.staging_path().to_owned();
            drop(prepared);
            assert!(!path.exists());
        })
        .await
        .unwrap();
    }
}
