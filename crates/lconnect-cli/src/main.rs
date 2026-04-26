//! `lconnectctl` — command-line client for the L-Connect Linux daemon.
//!
//! This binary is a thin wrapper around [`lconnect_ipc::client::IpcClient`].
//! Each subcommand opens a fresh connection (see [`connection::connect`]),
//! issues one or more JSON-RPC calls, and renders the response as either
//! human-readable text or JSON (via the global `--json` flag).
//!
//! The dispatch tree below is final for Phase 1. Per-subcommand bodies
//! land incrementally as P1E-2..P1E-7 — until then each stub returns
//! `anyhow::bail!` so individual commands fail loudly while the binary
//! itself builds and `--help` works end-to-end.

use clap::{Args, Parser, Subcommand};

mod commands;
mod connection;
mod format;

#[derive(Parser)]
#[command(version, about = "L-Connect Linux CLI")]
struct Cli {
    /// Output JSON instead of human-readable text (NDJSON for `watch`).
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List attached devices.
    Devices,
    /// Show firmware info for a device.
    Firmware {
        #[arg(long)]
        device: Option<String>,
    },
    /// Show the latest sensor snapshot for a device.
    Snapshot {
        #[arg(long)]
        device: Option<String>,
    },
    /// Stream sensor snapshots until interrupted.
    Watch {
        #[arg(long)]
        device: Option<String>,
    },
    /// Set fan PWM percent (0..=100).
    Fan {
        percent: u8,
        #[arg(long)]
        device: Option<String>,
    },
    /// Set pump PWM percent (0..=100).
    Pump {
        percent: u8,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        device: Option<String>,
    },
    /// Configure lighting (mode/colour/brightness/etc).
    Lighting(LightingArgs),
    /// Push a per-LED colour list.
    LedColors(LedColorsArgs),
    /// Configure the LCD (mode/brightness/rotation).
    Lcd(LcdArgs),
    /// Reset a device to factory defaults (destructive).
    Reset {
        #[arg(long)]
        yes_i_really_want_to_reset: bool,
        #[arg(long)]
        device: Option<String>,
    },
    /// Stream daemon logs (or print recent ones).
    Logs {
        #[arg(long)]
        follow: bool,
        #[arg(long)]
        level: Option<String>,
    },
    /// Send a raw HID frame (debugging).
    Raw {
        framing: String,
        hex: String,
        #[arg(long)]
        device: Option<String>,
    },
    /// Daemon-management subcommands (status, stats, etc).
    Daemon(DaemonArgs),
}

// The arg structs below are intentionally minimal placeholders. The
// owning P1E-* task fleshes each one out with its real flags; keeping
// them as `Option<String>` shaped today lets `main.rs` compile without
// blocking the parallel tasks on a shared schema.

/// Placeholder — populated in P1E-5.
#[derive(Args)]
pub struct LightingArgs {
    /// "fan" | "pump" — populated in P1E-5.
    #[arg(long)]
    pub kind: Option<String>,
}

/// Placeholder — populated in P1E-5.
#[derive(Args)]
pub struct LedColorsArgs {
    /// "fan" | "screen" — populated in P1E-5.
    #[arg(long)]
    pub target: Option<String>,
}

/// Placeholder — populated in P1E-6.
#[derive(Args)]
pub struct LcdArgs {
    /// e.g. "Application" | "LocalUI" — populated in P1E-6.
    #[arg(long)]
    pub mode: Option<String>,
}

/// Placeholder — populated in P1E-7.
#[derive(Args)]
pub struct DaemonArgs {
    /// e.g. "status" | "stats" — populated in P1E-7.
    #[arg(long)]
    pub action: Option<String>,
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    // Default to warn-level so a vanilla `lconnectctl` invocation stays
    // quiet on stderr; `RUST_LOG=...` overrides for debugging.
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("lconnectctl=warn"));

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        Cmd::Devices => commands::devices::run(cli.json).await,
        Cmd::Firmware { device } => commands::devices::firmware(device, cli.json).await,
        Cmd::Snapshot { device } => commands::devices::snapshot(device, cli.json).await,
        Cmd::Watch { device } => commands::watch::run(device, cli.json).await,
        Cmd::Fan { percent, device } => commands::pwm::fan(percent, device).await,
        Cmd::Pump {
            percent,
            source,
            device,
        } => commands::pwm::pump(percent, source, device).await,
        Cmd::Lighting(args) => commands::lighting::run(args).await,
        Cmd::LedColors(args) => commands::lighting::led_colors(args).await,
        Cmd::Lcd(args) => commands::lcd::run(args).await,
        Cmd::Reset {
            yes_i_really_want_to_reset,
            device,
        } => commands::maintenance::reset(yes_i_really_want_to_reset, device).await,
        Cmd::Logs { follow, level } => commands::maintenance::logs(follow, level).await,
        Cmd::Raw {
            framing,
            hex,
            device,
        } => commands::maintenance::raw(framing, hex, device).await,
        Cmd::Daemon(args) => commands::maintenance::daemon(args).await,
    }
}
