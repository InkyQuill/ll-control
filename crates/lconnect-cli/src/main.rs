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

/// `lconnectctl lighting <set|clear|info> ...` — see [`LightingSubcommand`].
#[derive(Args)]
pub struct LightingArgs {
    #[command(subcommand)]
    pub command: LightingSubcommand,
}

#[derive(clap::Subcommand)]
pub enum LightingSubcommand {
    /// Set lighting parameters.
    Set {
        /// fan or pump.
        kind: String,
        #[arg(long)]
        mode: String,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        speed: Option<u8>,
        #[arg(long)]
        brightness: Option<u8>,
        #[arg(long)]
        direction: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        disable: bool,
        #[arg(long)]
        device: Option<String>,
    },
    /// Clear lighting state on a kind.
    Clear {
        kind: String,
        #[arg(long)]
        device: Option<String>,
    },
    /// Print supported modes/parameters for a kind.
    Info {
        kind: String,
        #[arg(long)]
        device: Option<String>,
    },
}

/// `lconnectctl led-colors <fan|screen> [--color #RRGGBB --count N | --from-file PATH]`.
#[derive(Args)]
pub struct LedColorsArgs {
    /// fan or screen.
    pub target: String,
    /// Single color (#RRGGBB or rrggbb) applied to all LEDs.
    #[arg(long, conflicts_with = "from_file")]
    pub color: Option<String>,
    /// File with one #RRGGBB per line.
    #[arg(long)]
    pub from_file: Option<std::path::PathBuf>,
    /// Override count when --color is set (defaults to family-specific).
    #[arg(long)]
    pub count: Option<usize>,
    #[arg(long)]
    pub device: Option<String>,
}

/// `lconnectctl lcd <available|brightness|rotation|mode|image|clear> ...` —
/// see [`LcdSubcommand`].
#[derive(Args)]
pub struct LcdArgs {
    #[command(subcommand)]
    pub command: LcdSubcommand,
}

#[derive(clap::Subcommand)]
pub enum LcdSubcommand {
    /// Print whether the device exposes an LCD.
    Available {
        #[arg(long)]
        device: Option<String>,
    },
    /// Set LCD brightness (0..=100).
    ///
    /// NOTE: The Phase 1 IPC only exposes `set_lcd_config` wholesale, so
    /// this also overwrites mode (to Application) and rotation (to 0). A
    /// future `get_lcd_config` IPC method would let brightness be edited
    /// without disturbing siblings.
    Brightness {
        percent: u8,
        #[arg(long)]
        device: Option<String>,
    },
    /// Set LCD rotation (0/90/180/270).
    ///
    /// NOTE: same caveat as `brightness` — also overwrites mode and
    /// brightness.
    Rotation {
        degrees: u16,
        #[arg(long)]
        device: Option<String>,
    },
    /// Set LCD control mode (LocalUI, Application, ...).
    ///
    /// NOTE: same caveat as `brightness` — also overwrites brightness
    /// and rotation.
    Mode {
        mode: String,
        #[arg(long)]
        device: Option<String>,
    },
    /// Upload an image (any format ffmpeg can decode) as JPEG to the LCD.
    Image {
        path: std::path::PathBuf,
        /// Fit mode: crop | letterbox | stretch.
        #[arg(long, default_value = "crop")]
        fit: String,
        /// Letterbox background colour (#RRGGBB) — only used with --fit=letterbox.
        #[arg(long, default_value = "#000000")]
        letterbox_bg: String,
        /// JPEG quality 1..=100.
        #[arg(long, default_value_t = 85)]
        quality: u8,
        #[arg(long)]
        device: Option<String>,
    },
    /// Clear the displayed image (firmware reverts to LocalUI ~20s after).
    Clear {
        #[arg(long)]
        device: Option<String>,
    },
}

/// `lconnectctl daemon <status|stats|version>` — see [`DaemonSubcommand`].
#[derive(Args)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonSubcommand,
}

#[derive(clap::Subcommand)]
pub enum DaemonSubcommand {
    /// Print "OK" and the daemon's IPC version.
    Status,
    /// Print daemon stats counters.
    Stats,
    /// Print the IPC version the daemon advertises in its hello message.
    Version,
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
