//! Bounded one-shot query subprocess with timeout/reap quarantine.
use llcontrol_ipc::SensorStatus;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

pub(super) struct QueryProcess {
    executable: PathBuf,
    argument: &'static str,
    limit: usize,
    pub(super) draining: Option<Child>,
}
impl QueryProcess {
    pub(super) fn new(executable: PathBuf, argument: &'static str, limit: usize) -> Self {
        Self {
            executable,
            argument,
            limit,
            draining: None,
        }
    }
    pub(super) async fn query(&mut self) -> Result<Vec<u8>, SensorStatus> {
        query_command(
            Command::new(&self.executable).arg(self.argument),
            &mut self.draining,
            self.limit,
        )
        .await
    }
}

// Retain ownership even if collection is cancelled. A new query cannot overlap
// an unreaped helper, including one stuck in uninterruptible driver I/O.
pub(super) fn reap_finished(slot: &mut Option<Child>) -> bool {
    let Some(child) = slot.as_mut() else {
        return true;
    };
    if matches!(child.try_wait(), Ok(Some(_))) {
        *slot = None;
        true
    } else {
        let _ = child.start_kill();
        false
    }
}

pub(super) async fn query_command(
    command: &mut Command,
    slot: &mut Option<Child>,
    limit: usize,
) -> Result<Vec<u8>, SensorStatus> {
    if !reap_finished(slot) {
        return Err(SensorStatus::Unavailable);
    }
    *slot = Some(
        command
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| SensorStatus::Unavailable)?,
    );
    read_output(slot, limit).await
}

// Cancellation must request termination synchronously without dropping the
// provider's Child handle: Drop cannot wait for the kernel to reap it.
struct KillOnCancel<'a>(&'a mut Child);
impl Drop for KillOnCancel<'_> {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

pub(super) async fn read_output(
    slot: &mut Option<Child>,
    limit: usize,
) -> Result<Vec<u8>, SensorStatus> {
    let child = slot.as_mut().ok_or(SensorStatus::Unavailable)?;
    let guard = KillOnCancel(child);
    let stdout = guard.0.stdout.take().ok_or(SensorStatus::Unavailable)?;
    let operation = async {
        let mut bytes = Vec::new();
        stdout
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| SensorStatus::Unavailable)?;
        if bytes.len() > limit {
            return Err(SensorStatus::Invalid);
        }
        if !guard
            .0
            .wait()
            .await
            .map_err(|_| SensorStatus::Unavailable)?
            .success()
        {
            return Err(SensorStatus::Unavailable);
        }
        Ok(bytes)
    };
    let result = tokio::time::timeout(Duration::from_millis(750), operation)
        .await
        .unwrap_or(Err(SensorStatus::Unavailable));
    drop(guard);
    if result.is_ok()
        || matches!(
            tokio::time::timeout(Duration::from_millis(100), child.wait()).await,
            Ok(Ok(_))
        )
    {
        *slot = None;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_query_retains_child_until_reaped_before_retry() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("starts");
        let mut child = None;
        let mut command = Command::new("sh");
        command
            .args(["-c", "echo started >> \"$1\"; exec sleep 10", "fixture"])
            .arg(&marker);
        let mut pending = Box::pin(query_command(&mut command, &mut child, 1024));
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                tokio::select! {
                    result = &mut pending => panic!("query finished before cancellation: {result:?}"),
                    _ = tokio::time::sleep(Duration::from_millis(5)) => {
                        if marker.exists() { break; }
                    }
                }
            }
        }).await.unwrap();
        drop(pending);
        let pid = child
            .as_ref()
            .expect("cancelled query retains ownership")
            .id()
            .unwrap();
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "started\n");
        let mut recovery = Command::new("sh");
        recovery
            .args(["-c", "echo recovered >> \"$1\"; printf ready", "fixture"])
            .arg(&marker);
        let bytes = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match query_command(&mut recovery, &mut child, 1024).await {
                    Ok(bytes) => break bytes,
                    Err(SensorStatus::Unavailable) => {
                        tokio::time::sleep(Duration::from_millis(5)).await
                    }
                    other => panic!("unexpected query result: {other:?}"),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(bytes, b"ready");
        assert!(child.is_none());
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        assert_eq!(
            std::fs::read_to_string(marker).unwrap(),
            "started\nrecovered\n"
        );
    }

    #[tokio::test]
    async fn unreaped_child_refuses_spawn_without_waiting_for_exit() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("started");
        let mut slot = Some(
            Command::new("sleep")
                .arg("10")
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        );
        let pid = slot.as_ref().unwrap().id().unwrap();
        let mut command = Command::new("touch");
        command.arg(&marker);
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(100),
                query_command(&mut command, &mut slot, 1024)
            )
            .await
            .unwrap(),
            Err(SensorStatus::Unavailable)
        );
        assert!(
            !marker.exists(),
            "no new helper while old child is unreaped"
        );
        assert_eq!(slot.as_ref().unwrap().id(), Some(pid));
        tokio::time::timeout(Duration::from_secs(2), slot.as_mut().unwrap().wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            query_command(&mut command, &mut slot, 1024).await,
            Ok(vec![])
        );
        assert!(marker.exists());
    }
}
