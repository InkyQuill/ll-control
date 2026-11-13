//! IPC socket connection helper.
//!
//! Resolves the daemon socket path and returns a connected
//! [`lconnect_ipc::client::IpcClient`]. Honours the `LCONNECTCTL_SOCKET`
//! environment variable so integration tests and dev workflows can point
//! at an alternate socket without a config file. `LCONNECTCTL_CONNECT_TIMEOUT_MS`
//! and `LCONNECTCTL_RPC_TIMEOUT_MS` override the three-second connect/hello and
//! 30-second per-call deadlines respectively.

use anyhow::Context;
use std::time::Duration;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(30);

fn timeout_from_env(name: &str, default: Duration) -> anyhow::Result<Duration> {
    let Ok(raw) = std::env::var(name) else {
        return Ok(default);
    };
    let milliseconds = raw
        .parse::<u64>()
        .with_context(|| format!("{name} must be a positive integer"))?;
    if milliseconds == 0 {
        anyhow::bail!("{name} must be greater than zero");
    }
    Ok(Duration::from_millis(milliseconds))
}

/// Connect to the running daemon and return a ready-to-use IPC client.
///
/// The socket path is resolved as follows:
/// 1. If `LCONNECTCTL_SOCKET` is set, use that path verbatim.
/// 2. Otherwise, use `$XDG_RUNTIME_DIR/lconnectd.sock`.
///
/// # Errors
/// Returns an error if the runtime directory can't be resolved, the socket
/// can't be opened, either deadline is invalid, or connect/hello times out.
pub async fn connect() -> anyhow::Result<lconnect_ipc::client::IpcClient> {
    let socket = if let Ok(s) = std::env::var("LCONNECTCTL_SOCKET") {
        std::path::PathBuf::from(s)
    } else {
        lconnect_ipc::socket::default_socket_path()?
    };
    let connect_timeout =
        timeout_from_env("LCONNECTCTL_CONNECT_TIMEOUT_MS", DEFAULT_CONNECT_TIMEOUT)?;
    let rpc_timeout = timeout_from_env("LCONNECTCTL_RPC_TIMEOUT_MS", DEFAULT_RPC_TIMEOUT)?;
    let client = tokio::time::timeout(
        connect_timeout,
        lconnect_ipc::client::IpcClient::connect(&socket),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "daemon connect/hello timed out after {} ms; no RPC request was sent",
            connect_timeout.as_millis()
        )
    })??;
    Ok(client.with_call_timeout(rpc_timeout))
}

/// Resolve a [`lconnect_ipc::DeviceId`] for a per-device subcommand.
///
/// - `Some(id)` → wrap as [`lconnect_ipc::DeviceId`] verbatim; we let the
///   daemon validate it.
/// - `None` → call `list_devices` and pick the only attached device. If
///   none are attached, error with "no devices attached". If more than
///   one, error with the candidate list and ask the user to pass `--device`.
///
/// Single source of truth for device resolution across every CLI
/// subcommand. Standardises on the public [`lconnect_ipc::DeviceId::new`]
/// constructor so the inner field can be tightened to `pub(crate)` later
/// without breaking call sites.
pub async fn resolve_device(
    client: &lconnect_ipc::client::IpcClient,
    override_id: Option<String>,
) -> anyhow::Result<lconnect_ipc::DeviceId> {
    use anyhow::{anyhow, bail};
    use lconnect_ipc::{DeviceDescriptor, DeviceId};
    if let Some(s) = override_id {
        return Ok(DeviceId::new(s));
    }
    let v = client
        .call(
            lconnect_ipc::methods::LIST_DEVICES_NAME,
            serde_json::json!({}),
        )
        .await
        .map_err(|e| anyhow!("list_devices RPC failed: {e}"))?;
    let descs: Vec<DeviceDescriptor> = serde_json::from_value(v)
        .map_err(|e| anyhow!("list_devices response decode failed: {e}"))?;
    match descs.len() {
        0 => bail!("no devices attached"),
        1 => Ok(descs.into_iter().next().unwrap().id),
        _ => {
            let ids: Vec<String> = descs.iter().map(|d| d.id.as_str().to_string()).collect();
            Err(anyhow!(
                "ambiguous device; specify --device. Candidates: {}",
                ids.join(", "),
            ))
        }
    }
}
