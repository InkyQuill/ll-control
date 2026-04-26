//! Stub - body lands in P1E-7.

use crate::DaemonArgs;

pub async fn reset(
    _yes_i_really_want_to_reset: bool,
    _device: Option<String>,
) -> anyhow::Result<()> {
    anyhow::bail!("reset: not yet implemented (P1E-7)")
}

pub async fn logs(_follow: bool, _level: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!("logs: not yet implemented (P1E-7)")
}

pub async fn raw(_framing: String, _hex: String, _device: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!("raw: not yet implemented (P1E-7)")
}

pub async fn daemon(_args: DaemonArgs) -> anyhow::Result<()> {
    anyhow::bail!("daemon: not yet implemented (P1E-7)")
}
