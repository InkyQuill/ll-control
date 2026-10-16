//! Shared Unix socket path resolution for the daemon and its clients.

use std::io;
use std::path::{Path, PathBuf};

/// Return the well-known daemon socket inside an already-resolved runtime directory.
#[must_use]
pub fn socket_path_from_runtime(runtime: &Path) -> PathBuf {
    runtime.join("lconnectd.sock")
}

/// Resolve the daemon socket from `XDG_RUNTIME_DIR`.
///
/// # Errors
///
/// Returns an error when `XDG_RUNTIME_DIR` is unset, is not an absolute path,
/// or does not identify an existing directory.
pub fn default_socket_path() -> io::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "XDG_RUNTIME_DIR is unset; cannot resolve lconnectd socket",
        )
    })?;
    socket_path_for_runtime(&PathBuf::from(runtime))
}

fn socket_path_for_runtime(runtime: &Path) -> io::Result<PathBuf> {
    if !runtime.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "XDG_RUNTIME_DIR must be an absolute path, got {}",
                runtime.display()
            ),
        ));
    }

    let metadata = std::fs::metadata(runtime).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "XDG_RUNTIME_DIR does not identify an accessible directory ({}): {error}",
                runtime.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "XDG_RUNTIME_DIR must identify a directory, got {}",
                runtime.display()
            ),
        ));
    }

    Ok(socket_path_from_runtime(runtime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_runtime_rejects_relative_path() {
        let error = socket_path_for_runtime(Path::new("run/user/1000")).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn checked_runtime_rejects_missing_path() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing");

        let error = socket_path_for_runtime(&missing).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn checked_runtime_accepts_existing_absolute_directory() {
        let temp = tempfile::tempdir().unwrap();

        let socket = socket_path_for_runtime(temp.path()).unwrap();

        assert_eq!(socket, temp.path().join("lconnectd.sock"));
    }
}
