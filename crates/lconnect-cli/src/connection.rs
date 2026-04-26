//! IPC socket connection helper.
//!
//! Resolves the daemon socket path and returns a connected
//! [`lconnect_ipc::client::IpcClient`]. Honours the `LCONNECTCTL_SOCKET`
//! environment variable so integration tests and dev workflows can point
//! at an alternate socket without a config file.

/// Connect to the running daemon and return a ready-to-use IPC client.
///
/// The socket path is resolved as follows:
/// 1. If `LCONNECTCTL_SOCKET` is set, use that path verbatim.
/// 2. Otherwise, use `$XDG_RUNTIME_DIR/lconnectd/lconnectd.sock`.
///
/// # Errors
/// Returns an error if the runtime directory can't be resolved or the
/// socket can't be opened.
pub async fn connect() -> anyhow::Result<lconnect_ipc::client::IpcClient> {
    let socket = if let Ok(s) = std::env::var("LCONNECTCTL_SOCKET") {
        std::path::PathBuf::from(s)
    } else {
        let dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
        dirs.get_runtime_directory()?.join("lconnectd.sock")
    };
    Ok(lconnect_ipc::client::IpcClient::connect(&socket).await?)
}
