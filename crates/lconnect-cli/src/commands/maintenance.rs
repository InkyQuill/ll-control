//! Maintenance subcommands: `reset`, `logs`, `raw`, `daemon`.
//!
//! These are administrator-facing tools that sit alongside the regular
//! per-device controls. Each command opens a fresh IPC connection (where
//! one is needed at all — `logs` shells out to `journalctl --user` and
//! does not touch the daemon).
//!
//! `reset` is hard-guarded behind `--yes-i-really-want-to-reset` because
//! it factory-resets the device. `raw` is gated server-side on the
//! `[daemon] allow_raw_send` config flag; we surface a clear hint when
//! the daemon refuses with `NotImplemented`.

use crate::connection::{connect, resolve_device};
use crate::{DaemonArgs, DaemonSubcommand};
use lconnect_ipc::{IpcError, ResetToken};
use serde_json::{json, Value};

/// `lconnectctl reset --yes-i-really-want-to-reset [--device ID]`
///
/// Two-step destructive reset: mint a single-use token via
/// `request_reset_token`, then submit it to `reset_device`. The token
/// expires 30s after issuance and is consumed even if the reset itself
/// errors, so re-running this command always issues a fresh token.
pub async fn reset(yes_i_really_want_to_reset: bool, device: Option<String>) -> anyhow::Result<()> {
    if !yes_i_really_want_to_reset {
        anyhow::bail!(
            "refusing to reset without --yes-i-really-want-to-reset (this is destructive)"
        );
    }
    let client = connect().await?;
    let device_id = resolve_device(&client, device).await?;

    let v = client
        .call("request_reset_token", json!({ "dev": device_id }))
        .await?;
    let token: ResetToken = serde_json::from_value(v)?;
    println!("Token issued: {}. Sending reset_device...", token.0);

    client
        .call(
            "reset_device",
            json!({ "dev": device_id, "confirm": token }),
        )
        .await?;
    println!("Reset complete.");
    Ok(())
}

/// `lconnectctl logs [--follow] [--level LVL]`
///
/// Thin wrapper over `journalctl --user -u lconnectd --output=cat`.
/// `--level` is forwarded as `-p LVL`; journalctl accepts both numeric
/// (0..=7) and symbolic (`emerg`, `alert`, `crit`, `err`, `warning`,
/// `notice`, `info`, `debug`) priority names. We do not pre-validate the
/// value — journalctl produces a clear error if it rejects.
pub async fn logs(follow: bool, level: Option<String>) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("journalctl");
    cmd.arg("--user")
        .arg("-u")
        .arg("lconnectd")
        .arg("--output=cat");
    if follow {
        cmd.arg("--follow");
    }
    if let Some(lvl) = level {
        cmd.arg("-p").arg(lvl);
    }
    // status() inherits stdio so journalctl's output and any error
    // messages flow straight through to the terminal.
    let _ = cmd.status();
    Ok(())
}

/// `lconnectctl raw <a|b|c> <HEX> [--device ID]`
///
/// Forwards the parsed bytes to the daemon's `raw_send` IPC. The daemon
/// gates this on `[daemon] allow_raw_send` and otherwise replies with
/// `NotImplemented`; we translate that into an actionable hint instead
/// of the bare error string.
pub async fn raw(framing: String, hex: String, device: Option<String>) -> anyhow::Result<()> {
    if !matches!(framing.as_str(), "a" | "b" | "c") {
        anyhow::bail!("invalid framing: {framing} (allowed: a, b, c)");
    }
    let bytes = parse_hex(&hex)?;

    let client = connect().await?;
    let device_id = resolve_device(&client, device).await?;

    let result = client
        .call(
            "raw_send",
            json!({
                "dev": device_id,
                "framing": framing,
                "bytes": bytes,
            }),
        )
        .await;

    let value: Value = match result {
        Ok(v) => v,
        Err(IpcError::NotImplemented(_)) => {
            anyhow::bail!(
                "raw_send is disabled in daemon config. Set [daemon] allow_raw_send = true in daemon.toml to enable."
            );
        }
        Err(e) => return Err(e.into()),
    };

    let resp_bytes: Vec<u8> = serde_json::from_value(value)?;
    let printed: String = resp_bytes.iter().map(|b| format!("{b:02x}")).collect();
    println!("{printed}");
    Ok(())
}

/// `lconnectctl daemon <status|stats|version>`
///
/// All three subcommands open a fresh connection. `status` and `version`
/// only need the server-pushed `hello` notification (which the client
/// reads synchronously during `connect`); `stats` issues a single
/// `daemon_stats` RPC.
pub async fn daemon(args: DaemonArgs) -> anyhow::Result<()> {
    let client = connect().await?;
    match args.command {
        DaemonSubcommand::Status => {
            let ipc_version = client
                .hello
                .params
                .get("ipc_version")
                .and_then(Value::as_str)
                .unwrap_or("?");
            println!("OK (ipc_version: {ipc_version})");
        }
        DaemonSubcommand::Version => {
            let daemon_version = client
                .hello
                .params
                .get("daemon_version")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let ipc_version = client
                .hello
                .params
                .get("ipc_version")
                .and_then(Value::as_str)
                .unwrap_or("?");
            println!("daemon_version: {daemon_version}");
            println!("ipc_version: {ipc_version}");
        }
        DaemonSubcommand::Stats => {
            let v = client.call("daemon_stats", json!({})).await?;
            let stats: lconnect_ipc::DaemonStats = serde_json::from_value(v)?;
            println!("Uptime:        {}s", stats.uptime_seconds);
            println!("Devices:       {}", stats.devices_known);
            println!("HID writes:    {}", stats.hid_write_count);
            println!("HID read errs: {}", stats.hid_read_errors);
            println!("HID dec errs:  {}", stats.hid_decode_errors);
            println!("IPC requests:  {}", stats.ipc_request_count);
        }
    }
    Ok(())
}

/// Parse a hex string into bytes. Accepts whitespace and `:` as
/// separators (e.g. `"0102 03ff"`, `"01:02:03:ff"`). Errors on odd
/// length or non-hex characters.
fn parse_hex(s: &str) -> anyhow::Result<Vec<u8>> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    if cleaned.is_empty() {
        anyhow::bail!("empty hex input");
    }
    if cleaned.len() % 2 != 0 {
        anyhow::bail!("hex input has odd length: {}", cleaned.len());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> anyhow::Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => anyhow::bail!("invalid hex character: {:?}", b as char),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_plain() {
        assert_eq!(parse_hex("0102ff").unwrap(), vec![0x01, 0x02, 0xff]);
    }

    #[test]
    fn parse_hex_with_whitespace() {
        assert_eq!(parse_hex("01 02 ff").unwrap(), vec![0x01, 0x02, 0xff]);
        assert_eq!(
            parse_hex("0102 03ff").unwrap(),
            vec![0x01, 0x02, 0x03, 0xff]
        );
    }

    #[test]
    fn parse_hex_with_colons() {
        assert_eq!(
            parse_hex("01:02:03:ff").unwrap(),
            vec![0x01, 0x02, 0x03, 0xff]
        );
    }

    #[test]
    fn parse_hex_uppercase() {
        assert_eq!(parse_hex("0A0B0C").unwrap(), vec![0x0a, 0x0b, 0x0c]);
    }

    #[test]
    fn parse_hex_rejects_odd_length() {
        assert!(parse_hex("0102f").is_err());
    }

    #[test]
    fn parse_hex_rejects_non_hex() {
        assert!(parse_hex("01zz").is_err());
    }

    #[test]
    fn parse_hex_rejects_empty() {
        assert!(parse_hex("").is_err());
        assert!(parse_hex("   ").is_err());
    }
}
