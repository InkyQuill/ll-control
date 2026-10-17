//! Phase-0 test harness for Galahad II LCDV2.
//!
//! Not a product binary. This exists only to exercise commands defined
//! in `lconnect-protocol` against a live device and print the results
//! into `docs/development/protocol/galahad2-lcdv2.md`'s "Verification" sections.
//!
//! Once Phase 1 stands up the real daemon, this tool is retired.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use hidapi::{HidApi, HidDevice};

/// VID/PID pair for Galahad II LCDV2.
const GALAHAD2_LCDV2_VID: u16 = 0x0416;
const GALAHAD2_LCDV2_PID: u16 = 0x7395;

/// Phase-0 test harness for Galahad II LCDV2.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List Lian Li devices visible to hidapi (matches Galahad II LCDV2 VID/PID).
    List,
    /// Send GetHandshakeInfo (cmd 129) and decode the response.
    Handshake,
    /// Send GetFirmwareVersion (cmd 134) and decode the two-packet response.
    Firmware,
    /// Send SetFanPWM (cmd 139). NOTE: physically changes fan speed.
    Fan {
        /// Percent duty cycle, 0..=100. Values outside this range are rejected.
        #[arg(long)]
        percent: u8,
    },
    /// Send SetPumpPWM (cmd 138). NOTE: physically changes pump speed —
    /// running the pump too low can cause the AIO to overheat.
    Pump {
        /// Percent duty cycle, 0..=100. Setting < 30 is risky on most builds.
        #[arg(long)]
        percent: u8,
        /// Who controls the pump: `mcu` (firmware curve) or `motherboard` (MB PWM signal).
        #[arg(long, default_value = "mcu")]
        source: PumpSourceArg,
    },
    /// Send ResetDevice (cmd 142). DANGEROUS — drops pump for 1–3s.
    Reset {
        /// Required to actually run; without it, the command refuses.
        #[arg(long)]
        yes_i_really_want_to_reset: bool,
    },
    /// Send GetPumpLightingInfo (cmd 130) and decode the response.
    GetPumpLighting,
    /// Send SetPumpLighting (cmd 131). NOTE: physically changes pump LEDs.
    SetPumpLighting(SetPumpLightingArgs),
    /// Send GetFanLightingInfo (cmd 132) and decode the response.
    GetFanLighting,
    /// Send SetFanLighting (cmd 133). NOTE: physically changes fan LEDs.
    SetFanLighting(SetFanLightingArgs),
    /// Send SetLEDColors (cmd 20). NOTE: physically changes LED colours
    /// on either the screen / pump-cap ring or the fan ring.
    SetLedColors(SetLedColorsArgs),
    /// Send a JPEG to the pump-cap LCD via cmd 14 (SendJpeg) or cmd 15
    /// (UploadJpeg). NOTE: physically changes the LCD content.
    UploadJpeg(UploadJpegArgs),
    /// Send a single read-style request to the device using a chosen
    /// framing class and dump the hex response. For Phase 0e cross-device
    /// experimentation; the caller is responsible for choosing a cmd byte
    /// that has been hid-safety-reviewer-cleared as read-only. See
    /// `docs/development/protocol/captures/2026-04-26-cross-device-probes.txt` for
    /// the curated probe set.
    Probe(ProbeCliArgs),
}

/// Flags for `probe`. Distinct from
/// [`verify_galahad::probe::ProbeArgs`] (the codec-layer struct that
/// `run_probe` consumes) so the CLI can hold pre-parse state (the raw
/// `--payload` string, the `--yes-read-only` gate) without polluting
/// the codec layer.
#[derive(Args, Debug)]
struct ProbeCliArgs {
    /// Command byte to send (decimal `134` or hex `0x86`).
    #[arg(value_parser = parse_cmd_byte_arg)]
    cmd: u8,
    /// HID framing class.
    #[arg(long, default_value = "a")]
    framing: verify_galahad::probe::Framing,
    /// Hex payload to send after the cmd byte (defaults to empty).
    #[arg(long, default_value = "")]
    payload: String,
    /// Acknowledge that this probe has been hid-safety-reviewer-cleared
    /// as read-only.
    #[arg(long)]
    yes_read_only: bool,
}

/// Flags for `set-pump-lighting`.
///
/// `--brightness` and `--speed` are user-facing 0..=100 percentages; the
/// firmware wire range is 0..=4. The CLI maps percent -> wire by integer
/// division by 25, matching `Galahad2VisionController.cs:1290-1291`. Only
/// color1 is exposed; color2..4 default to black (Breathing-style modes
/// will appear single-colour as a result).
#[derive(Args, Debug)]
struct SetPumpLightingArgs {
    /// Lighting mode (effect). Defaults to a solid colour.
    #[arg(long, default_value = "static-color")]
    mode: PumpModeArg,
    /// Primary colour as `#RRGGBB` (six hex digits, optional leading `#`).
    #[arg(long, default_value = "#FF0000", value_parser = parse_hex_color)]
    color: lconnect_protocol::galahad2_lcdv2::lighting::Rgb,
    /// Brightness percent, 0..=100. Mapped to wire bytes 0..=4 (percent / 25).
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u8).range(0..=100))]
    brightness: u8,
    /// Animation speed percent, 0..=100. Mapped to wire bytes 0..=4 (percent / 25).
    /// Use 0 for non-animated modes; animated modes (Breathing, Runway, Meteor,
    /// Ticker-Tape, Bounce, ...) need a non-zero speed to appear in motion.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=100))]
    speed: u8,
    /// Which LED ring to target.
    #[arg(long, default_value = "inner")]
    scope: PumpScopeArg,
    /// Where the ARGB signal comes from.
    #[arg(long, default_value = "mcu")]
    source: ArgbSourceArg,
    /// Direction of motion-based effects.
    #[arg(long, default_value = "right")]
    direction: DirectionArg,
    /// Switch the pump LEDs off entirely.
    #[arg(long)]
    disable: bool,
}

/// Flags for `set-fan-lighting`. See `SetPumpLightingArgs` doc for the
/// brightness/speed wire-mapping rationale.
#[derive(Args, Debug)]
struct SetFanLightingArgs {
    /// Lighting mode (effect). Defaults to a solid colour.
    #[arg(long, default_value = "static-color")]
    mode: FanModeArg,
    /// Primary colour as `#RRGGBB` (six hex digits, optional leading `#`).
    #[arg(long, default_value = "#FF0000", value_parser = parse_hex_color)]
    color: lconnect_protocol::galahad2_lcdv2::lighting::Rgb,
    /// Brightness percent, 0..=100. Mapped to wire bytes 0..=4 (percent / 25).
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u8).range(0..=100))]
    brightness: u8,
    /// Animation speed percent, 0..=100. Mapped to wire bytes 0..=4 (percent / 25).
    /// Use 0 for non-animated modes.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=100))]
    speed: u8,
    /// Where the ARGB signal comes from.
    #[arg(long, default_value = "mcu")]
    source: ArgbSourceArg,
    /// Direction of motion-based effects.
    #[arg(long, default_value = "right")]
    direction: DirectionArg,
    /// Switch the fan LEDs off entirely.
    #[arg(long)]
    disable: bool,
    /// Mirror pump lighting onto the fans.
    #[arg(long)]
    sync_to_pump: bool,
    /// Number of LEDs in the fan ring. Known shipping values are 16 and 24;
    /// out-of-range values are rejected because the firmware uses this to
    /// bound its LED-ring write loop.
    #[arg(long, default_value_t = 24, value_parser = clap::value_parser!(u8).range(1..=60))]
    number_of_led: u8,
}

/// Flags for `set-led-colors`.
///
/// Drives `SetLEDColors` (cmd 20) directly: writes a uniform colour
/// across `count` LEDs on either the screen / pump-cap ring or the fan
/// ring.
///
/// `--count` is capped at 80 even though the codec accepts up to
/// `MAX_SCREEN_LEDS=329` / `MAX_FAN_LEDS=337`. The physical Galahad II
/// LCDV2 ring is small (~16-24 LEDs); the conservative cap prevents a
/// typo from writing far more LEDs than hardware exists, where firmware
/// behaviour for over-addressed indices is unverified.
///
/// The `--check-available` and `--set-application-mode` flags are
/// optional preflights for the case where a bare `set-led-colors` run
/// produces no visible change. As of Phase 0c, cmd 131 (`SetPumpLighting`)
/// renders without `Application` mode, so cmd 20 may be the same — but
/// the host-driven mode hasn't been live-verified for cmd 20 yet.
#[derive(Args, Debug)]
struct SetLedColorsArgs {
    /// Primary colour as `#RRGGBB`.
    #[arg(long, value_parser = parse_hex_color)]
    color: lconnect_protocol::galahad2_lcdv2::Rgb,

    /// Number of LEDs to address. Default 16 (matches the LCDV2 pump-cap
    /// ring). Capped at 80 (well below `MAX_SCREEN_LEDS=329` /
    /// `MAX_FAN_LEDS=337`) to prevent typo-driven over-writes on small
    /// hardware rings — Galahad II LCDV2 ships with ~16-24 LEDs per ring.
    #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u8).range(1..=80))]
    count: u8,

    /// Which LED ring to target.
    #[arg(long, default_value = "screen")]
    kind: LedKindArg,

    /// FOR TESTING ONLY — skip the user-consent prompt. Do not use in
    /// scripts that run unattended against production hardware.
    #[arg(long)]
    yes: bool,

    /// Send IsLCDAvailable first; abort if the device reports unavailable.
    #[arg(long)]
    check_available: bool,

    /// Send LCDControl{mode=Application} before the write to put the
    /// firmware into host-driven mode. Whether this is necessary on the
    /// LCDV2 is currently unknown — the Phase 0c session showed cmd 131
    /// renders without it; cmd 20 may be the same. Use this if a bare
    /// set-led-colors run produces no visible LED change.
    #[arg(long)]
    set_application_mode: bool,
}

/// Flags for `upload-jpeg`.
///
/// Drives `SendJpeg` (cmd 14) or `UploadJpeg` (cmd 15) end-to-end:
/// `lconnect-image::prepare_jpeg_from_path` decodes any input format
/// the `image` crate supports (PNG, JPEG, WebP, BMP, GIF), fits it to
/// 480×480 per `--fit`, re-encodes as JPEG at `--quality`, and the
/// chosen codec packetises the bytes for the device.
///
/// `--mode send` is the default because the C# enum naming suggests
/// `SendJpeg` is the transient-display path (lower flash-cycle risk)
/// and `UploadJpeg` is the persisted-storage path. The wire shapes are
/// presumed identical except for the cmd byte; `--mode` is the live-
/// test knob to discover the firmware's behavioural difference.
///
/// The `--check-available` and `--set-application-mode` flags mirror
/// `set-led-colors`: they're optional preflights for the case where a
/// bare `upload-jpeg` run produces no visible LCD change. As of
/// Phase 0c, cmd 20 renders without `Application` mode; whether
/// cmd 14/15 also do is empirically unverified.
#[derive(Args, Debug)]
struct UploadJpegArgs {
    /// Input image (any format the `image` crate decodes: PNG, JPEG,
    /// WebP, BMP, GIF). The image is resized/cropped to 480×480 and
    /// re-encoded as JPEG before transmission.
    #[arg(long)]
    path: std::path::PathBuf,

    /// Which JPEG codec to invoke on the wire. `send` (cmd 14) is the
    /// presumed transient-display path; `upload` (cmd 15) is the
    /// presumed persisted-storage path. The wire shapes are identical
    /// except for the cmd byte; the live test discovers the firmware's
    /// behavioural difference.
    #[arg(long, default_value = "send")]
    mode: JpegModeArg,

    /// How to fit a non-480×480 source image into the LCD frame.
    #[arg(long, default_value = "crop")]
    fit: FitModeArg,

    /// JPEG quality, 1..=100. Default 90 (matches lconnect-image's default).
    /// Higher quality (95+) bloats packet count without much visible improvement
    /// on the 480×480 LCD; lower quality (≤70) loses fine detail in natural
    /// imagery. 90 is a sane mid-default.
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,

    /// Background colour as `#RRGGBB`, used only when `--fit letterbox`.
    #[arg(long, default_value = "#000000", value_parser = parse_hex_color)]
    letterbox_bg: lconnect_protocol::galahad2_lcdv2::Rgb,

    /// FOR TESTING ONLY — skip the user-consent prompt. Do not use in
    /// scripts that run unattended against production hardware.
    #[arg(long)]
    yes: bool,

    /// Send IsLCDAvailable first; abort if the device reports unavailable.
    #[arg(long)]
    check_available: bool,

    /// Send LCDControl{mode=Application} before the upload, in case the
    /// firmware needs host-driven mode active. Whether this is needed
    /// for cmd 14/15 is currently unknown — the Phase 0c set-led-colors
    /// session showed cmd 20 renders without it; cmd 14/15 may differ.
    #[arg(long)]
    set_application_mode: bool,
}

/// CLI-side rendering of the cmd 14 vs cmd 15 codec choice.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum JpegModeArg {
    /// `SendJpeg` (cmd 14) — presumed transient frame display.
    Send,
    /// `UploadJpeg` (cmd 15) — presumed persisted-storage path.
    Upload,
}

impl JpegModeArg {
    /// User-facing lowercase form, matching what the user typed on the
    /// command line. Use this in log/error output instead of `{:?}` so
    /// messages echo "send"/"upload" rather than "Send"/"Upload".
    fn as_str(self) -> &'static str {
        match self {
            JpegModeArg::Send => "send",
            JpegModeArg::Upload => "upload",
        }
    }
}

/// CLI-side rendering of the resize/fit policy.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum FitModeArg {
    /// Scale uniformly to cover, then centre-crop the excess.
    Crop,
    /// Scale uniformly to fit, fill remainder with `--letterbox-bg`.
    Letterbox,
    /// Stretch to 480×480 (may distort aspect ratio).
    Stretch,
}

impl FitModeArg {
    /// User-facing lowercase form. See `JpegModeArg::as_str` for rationale.
    fn as_str(self) -> &'static str {
        match self {
            FitModeArg::Crop => "crop",
            FitModeArg::Letterbox => "letterbox",
            FitModeArg::Stretch => "stretch",
        }
    }
}

/// Convert the CLI fit-mode arg + parsed letterbox background into the
/// `lconnect-image::FitMode` the codec expects. The background colour is
/// only meaningful for `Letterbox`, but it's parsed unconditionally so we
/// pair them at the conversion boundary.
///
/// This is a free function rather than a `From` impl because the orphan
/// rule blocks `impl From<(LocalArg, ForeignRgb)> for ForeignFitMode` —
/// the tuple's foreignness wins over the local `FitModeArg` element, so
/// neither the trait nor the `Self` type is local.
fn fit_mode_from_args(
    arg: FitModeArg,
    bg: lconnect_protocol::galahad2_lcdv2::Rgb,
) -> lconnect_image::FitMode {
    match arg {
        FitModeArg::Crop => lconnect_image::FitMode::Crop,
        FitModeArg::Letterbox => lconnect_image::FitMode::Letterbox {
            bg: [bg.r, bg.g, bg.b],
        },
        FitModeArg::Stretch => lconnect_image::FitMode::Stretch,
    }
}

/// CLI-side rendering of the screen-vs-fan LED-ring choice.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum LedKindArg {
    /// Screen / pump-cap ring (`build_set_screen_led_colors_request`,
    /// `isFanLed=false` payload, 24-byte zero pad).
    Screen,
    /// Fan ring (`build_set_fan_led_colors_request`, `isFanLed=true`
    /// payload).
    Fan,
}

/// CLI-side rendering of `PumpPwmSource`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PumpSourceArg {
    Mcu,
    Motherboard,
}

impl From<PumpSourceArg> for lconnect_protocol::galahad2_lcdv2::pump_pwm::PumpPwmSource {
    fn from(v: PumpSourceArg) -> Self {
        use lconnect_protocol::galahad2_lcdv2::pump_pwm::PumpPwmSource as P;
        match v {
            PumpSourceArg::Mcu => P::Mcu,
            PumpSourceArg::Motherboard => P::Motherboard,
        }
    }
}

/// CLI-side rendering of `PumpLightingMode`. `Unknown` is intentionally
/// not exposed (it would never round-trip through the device).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PumpModeArg {
    Rainbow,
    RainbowMorph,
    StaticColor,
    Breathing,
    Runway,
    Meteor,
    TickerTape,
    Fluctuation,
    Transmit,
    ColorfulStarryNight,
    StaticStarryNight,
    Voice,
    BigBang,
    Burst,
    ColorsMorph,
    Bounce,
}

impl From<PumpModeArg> for lconnect_protocol::galahad2_lcdv2::lighting_enums::PumpLightingMode {
    fn from(v: PumpModeArg) -> Self {
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::PumpLightingMode as P;
        match v {
            PumpModeArg::Rainbow => P::Rainbow,
            PumpModeArg::RainbowMorph => P::RainbowMorph,
            PumpModeArg::StaticColor => P::StaticColor,
            PumpModeArg::Breathing => P::Breathing,
            PumpModeArg::Runway => P::Runway,
            PumpModeArg::Meteor => P::Meteor,
            PumpModeArg::TickerTape => P::TickerTape,
            PumpModeArg::Fluctuation => P::Fluctuation,
            PumpModeArg::Transmit => P::Transmit,
            PumpModeArg::ColorfulStarryNight => P::ColorfulStarryNight,
            PumpModeArg::StaticStarryNight => P::StaticStarryNight,
            PumpModeArg::Voice => P::Voice,
            PumpModeArg::BigBang => P::BigBang,
            PumpModeArg::Burst => P::Burst,
            PumpModeArg::ColorsMorph => P::ColorsMorph,
            PumpModeArg::Bounce => P::Bounce,
        }
    }
}

/// CLI-side rendering of `FanLightingMode`. `Unknown` is intentionally
/// not exposed.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum FanModeArg {
    Rainbow,
    RainbowMorph,
    StaticColor,
    Breathing,
    Runway,
    Meteor,
}

impl From<FanModeArg> for lconnect_protocol::galahad2_lcdv2::lighting_enums::FanLightingMode {
    fn from(v: FanModeArg) -> Self {
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::FanLightingMode as F;
        match v {
            FanModeArg::Rainbow => F::Rainbow,
            FanModeArg::RainbowMorph => F::RainbowMorph,
            FanModeArg::StaticColor => F::StaticColor,
            FanModeArg::Breathing => F::Breathing,
            FanModeArg::Runway => F::Runway,
            FanModeArg::Meteor => F::Meteor,
        }
    }
}

/// CLI-side rendering of `PumpLightingScope`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PumpScopeArg {
    Inner,
    Outer,
    All,
}

impl From<PumpScopeArg> for lconnect_protocol::galahad2_lcdv2::lighting_enums::PumpLightingScope {
    fn from(v: PumpScopeArg) -> Self {
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::PumpLightingScope as S;
        match v {
            PumpScopeArg::Inner => S::Inner,
            PumpScopeArg::Outer => S::Outer,
            PumpScopeArg::All => S::All,
        }
    }
}

/// CLI-side rendering of `ArgbSignalSource`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ArgbSourceArg {
    Mcu,
    Motherboard,
    Application,
}

impl From<ArgbSourceArg> for lconnect_protocol::galahad2_lcdv2::lighting_enums::ArgbSignalSource {
    fn from(v: ArgbSourceArg) -> Self {
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::ArgbSignalSource as A;
        match v {
            ArgbSourceArg::Mcu => A::Mcu,
            ArgbSourceArg::Motherboard => A::Motherboard,
            ArgbSourceArg::Application => A::Application,
        }
    }
}

/// CLI-side rendering of `LightingDirection`. The protocol enum has two
/// composite names (`RightOrClockwise`, `LeftOrCounterClockwise`); the CLI
/// exposes them as plain `right`/`left` to match the other axis names.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DirectionArg {
    Right,
    Left,
    Up,
    Down,
    Spread,
    Gather,
}

impl From<DirectionArg> for lconnect_protocol::galahad2_lcdv2::lighting_enums::LightingDirection {
    fn from(v: DirectionArg) -> Self {
        use lconnect_protocol::galahad2_lcdv2::lighting_enums::LightingDirection as D;
        match v {
            DirectionArg::Right => D::RightOrClockwise,
            DirectionArg::Left => D::LeftOrCounterClockwise,
            DirectionArg::Up => D::Up,
            DirectionArg::Down => D::Down,
            DirectionArg::Spread => D::Spreaded,
            DirectionArg::Gather => D::Gathered,
        }
    }
}

/// Parse a `u8` from decimal (`134`) or `0x`-prefixed hex (`0x86`).
///
/// Used by `probe`'s positional `<cmd>` argument so that captures and
/// docs can be quoted in either form. Returns `String` (not `anyhow`)
/// because clap value parsers require `Result<T, impl Display>`.
fn parse_cmd_byte_arg(s: &str) -> Result<u8, String> {
    let s = s.trim();
    let (radix, body) = if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (16, rest)
    } else {
        (10, s)
    };
    u8::from_str_radix(body, radix).map_err(|e| format!("invalid cmd byte {s:?}: {e}"))
}

/// Parse a `#RRGGBB` (or bare `RRGGBB`) string into an `Rgb`.
fn parse_hex_color(s: &str) -> Result<lconnect_protocol::galahad2_lcdv2::lighting::Rgb> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return Err(anyhow!(
            "invalid --color {s:?}: expected 6 hex digits (e.g. #FF0000), got {} digits",
            hex.len(),
        ));
    }
    let n = u32::from_str_radix(hex, 16)
        .map_err(|e| anyhow!("invalid --color {s:?}: not valid hex: {e}"))?;
    Ok(lconnect_protocol::galahad2_lcdv2::lighting::Rgb {
        r: ((n >> 16) & 0xFF) as u8,
        g: ((n >> 8) & 0xFF) as u8,
        b: (n & 0xFF) as u8,
    })
}

/// Read packets from `device` until one arrives whose Report-ID/Command
/// echo matches `expected_cmd`, draining stale buffered responses from
/// previous CLI invocations.
///
/// The Galahad II LCDV2 (FW 1.6) buffers exactly one response per
/// open/close cycle, so the FIRST `read_timeout` after opening the
/// device frequently returns a stale packet from a prior `verify-galahad`
/// run (e.g. opening for `pump-info` returns the leftover `fan-info`
/// response). Without draining, the subsequent `parse_response` call
/// raises `UnexpectedCommand { expected: 0x84, got: 0x82 }`.
///
/// This helper retries the read on echo mismatches until either the
/// expected command arrives or the cumulative deadline (`deadline_ms`)
/// is exhausted. Stale packets are reported on stderr and dropped.
fn read_until_command<const N: usize>(
    device: &HidDevice,
    expected_cmd: u8,
    deadline_ms: i32,
) -> Result<[u8; N]> {
    let start = std::time::Instant::now();
    loop {
        let elapsed: i32 = start.elapsed().as_millis().try_into().unwrap_or(i32::MAX);
        let remaining = deadline_ms.saturating_sub(elapsed);
        if remaining <= 0 {
            return Err(anyhow!(
                "read timeout draining stale responses (expected cmd 0x{expected_cmd:02x})"
            ));
        }
        let mut buf = [0u8; N];
        let _read = device
            .read_timeout(&mut buf, remaining)
            .context("HID read_timeout failed")?;
        if buf.first() == Some(&0x01) && buf.get(1) == Some(&expected_cmd) {
            return Ok(buf);
        }
        eprintln!(
            "  drained stale response (cmd 0x{:02x}), retrying...",
            buf.get(1).copied().unwrap_or(0)
        );
    }
}

/// One-shot 512-byte BCommandPacket read. B-class responses don't have
/// the cmd-0x8e unsolicited-packet quirk that motivates `read_until_command`
/// for A-class reads, so a single `read_timeout` call is sufficient.
fn read_b_packet(
    device: &HidDevice,
    timeout_ms: i32,
) -> Result<[u8; lconnect_protocol::galahad2_lcdv2::bpacket::INPUT_LEN]> {
    use lconnect_protocol::galahad2_lcdv2::bpacket::INPUT_LEN;
    let mut buf = [0u8; INPUT_LEN];
    let _read = device
        .read_timeout(&mut buf, timeout_ms)
        .context("HID read_timeout failed")?;
    Ok(buf)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::List => list_devices(),
        Cmd::Handshake => handshake(),
        Cmd::Firmware => firmware(),
        Cmd::Fan { percent } => fan(percent),
        Cmd::Pump { percent, source } => pump(percent, source.into()),
        Cmd::Reset {
            yes_i_really_want_to_reset,
        } => reset_device(yes_i_really_want_to_reset),
        Cmd::GetPumpLighting => get_pump_lighting(),
        Cmd::SetPumpLighting(args) => set_pump_lighting(args),
        Cmd::GetFanLighting => get_fan_lighting(),
        Cmd::SetFanLighting(args) => set_fan_lighting(args),
        Cmd::SetLedColors(args) => set_led_colors(args),
        Cmd::UploadJpeg(args) => upload_jpeg(args),
        Cmd::Probe(args) => probe(args),
    }
}

fn probe(args: ProbeCliArgs) -> Result<()> {
    if !args.yes_read_only {
        return Err(anyhow!(
            "an arbitrary cmd byte may mutate device state; \
             refusing to probe without --yes-read-only. \
             Confirm cmd byte 0x{:02x} has been hid-safety-reviewer-cleared as \
             read-only and re-run with --yes-read-only",
            args.cmd
        ));
    }

    let payload = verify_galahad::probe::parse_hex_payload(&args.payload)?;
    let probe_args = verify_galahad::probe::ProbeArgs {
        cmd: args.cmd,
        framing: args.framing,
        payload,
    };

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    println!(
        "probe target: vid=0x{:04x} pid=0x{:04x}",
        GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID
    );
    println!(
        "probe cmd=0x{:02x} framing={:?} payload={} bytes",
        probe_args.cmd,
        probe_args.framing,
        probe_args.payload.len()
    );

    let response = verify_galahad::probe::run_probe(&device, &probe_args)?;
    println!("response ({} bytes):", response.len());
    print_hex_dump(&response);
    Ok(())
}

/// Dump `bytes` as a 16-bytes-per-row hex grid, one row prefixed with
/// its byte offset. Mirrors the inline format used by `handshake`,
/// `firmware`, et al., but extracted so `probe` (which doesn't know
/// `bytes.len()` at compile time) can share the same look.
fn print_hex_dump(bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        if i % 16 == 0 {
            if i > 0 {
                println!();
            }
            print!("  {:04}: ", i);
        }
        print!("{:02x} ", b);
    }
    println!();
}

fn list_devices() -> Result<()> {
    let api = HidApi::new().context("HidApi::new failed")?;
    let mut found = 0usize;
    for info in api.device_list() {
        if info.vendor_id() == GALAHAD2_LCDV2_VID && info.product_id() == GALAHAD2_LCDV2_PID {
            found += 1;
            println!(
                "vid=0x{:04x} pid=0x{:04x}  path={:?}  manufacturer={:?}  product={:?}  serial={:?}  interface={}",
                info.vendor_id(),
                info.product_id(),
                info.path(),
                info.manufacturer_string(),
                info.product_string(),
                info.serial_number(),
                info.interface_number(),
            );
        }
    }
    if found == 0 {
        return Err(anyhow!(
            "no Galahad II LCDV2 (0x{:04x}:0x{:04x}) found by hidapi. \
             Check `lsusb`, plug the device in, and re-run.",
            GALAHAD2_LCDV2_VID,
            GALAHAD2_LCDV2_PID,
        ));
    }
    Ok(())
}

fn handshake() -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::commands::Command;
    use lconnect_protocol::galahad2_lcdv2::handshake::{build_request, HandshakeInfo};
    use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    // Send the 64-byte request.
    let req = build_request();
    let written = device.write(&req).context("HID write failed")?;
    println!(
        "wrote {} bytes (expect 65 incl. report id, or 64 depending on backend)",
        written
    );

    // Read until the expected command echo arrives (draining any stale
    // packets buffered from a previous CLI invocation).
    let buf: [u8; PACKET_LEN] = read_until_command(&device, Command::GetHandshakeInfo as u8, 1000)?;
    println!("read {} bytes", PACKET_LEN);

    // Print the raw bytes for the protocol doc.
    print!("raw response:");
    for (i, b) in buf.iter().enumerate() {
        if i % 16 == 0 {
            print!("\n  {:02}: ", i);
        }
        print!("{:02x} ", b);
    }
    println!();

    // Decode using the protocol crate.
    match HandshakeInfo::from_response(&buf) {
        Ok(info) => {
            println!();
            println!("decoded:");
            println!("  fan_rpm       = {}", info.fan_rpm);
            println!("  pump_rpm      = {}", info.pump_rpm);
            println!("  temp_valid    = {}", info.temp_valid);
            println!("  temp_integer  = {}", info.temp_integer);
            println!("  temp_fraction = {}", info.temp_fraction);
            println!("  temperature_c = {:.1}", info.temperature_c());
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}\nraw bytes printed above")),
    }
}

fn firmware() -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::commands::Command;
    use lconnect_protocol::galahad2_lcdv2::firmware::FirmwareVersion;
    use lconnect_protocol::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    // Drain any stale packets queued in the device's input buffer from
    // previous interactions or unsolicited notifications. Without this,
    // the read loop below can pick up leftover packets (e.g., a 0x8e
    // status packet observed on FW 1.6 between commands) and miss the
    // packet we actually want.
    let mut drained = 0usize;
    loop {
        let mut scratch = [0u8; PACKET_LEN];
        match device.read_timeout(&mut scratch, 50) {
            Ok(n) if n > 0 => {
                drained += 1;
                eprintln!(
                    "(drained stale packet: {n} bytes, cmd=0x{:02x}, packet_num={})",
                    scratch[1],
                    u16::from_be_bytes([scratch[3], scratch[4]]),
                );
            }
            _ => break,
        }
    }
    if drained > 0 {
        println!("(drained {drained} stale packet(s) before sending firmware request)");
    }

    // Send the request.
    let req = *ACommandPacket::new_request(Command::GetFirmwareVersion as u8, &[]).as_bytes();
    let written = device.write(&req).context("HID write failed")?;
    println!("wrote {written} bytes");

    // Collect cmd-134 packets indexed by their PacketNumber (byte[3..4]).
    // The decompiled C# expected exactly 2 sequential reads but didn't
    // filter — on real FW 1.6 hardware, packet ordering is by
    // PacketNumber, not by read order, and unsolicited packets may be
    // interleaved. Read up to 6 packets within ~1s and pick the right
    // ones by both cmd echo and PacketNumber.
    let cmd_fw = Command::GetFirmwareVersion as u8;
    let mut version_pkt: Option<[u8; PACKET_LEN]> = None;
    let mut datetime_pkt: Option<[u8; PACKET_LEN]> = None;
    let mut all_seen: Vec<[u8; PACKET_LEN]> = Vec::new();

    for i in 0..6 {
        let mut buf = [0u8; PACKET_LEN];
        match device.read_timeout(&mut buf, if i == 0 { 1000 } else { 200 }) {
            Ok(n) if n > 0 => {
                let cmd = buf[1];
                let pn = u16::from_be_bytes([buf[3], buf[4]]);
                println!(
                    "read packet {} ({n} bytes, cmd=0x{cmd:02x}, packet_num={pn})",
                    all_seen.len() + 1,
                );
                all_seen.push(buf);
                if cmd == cmd_fw {
                    match pn {
                        0 if version_pkt.is_none() => version_pkt = Some(buf),
                        1 if datetime_pkt.is_none() => datetime_pkt = Some(buf),
                        _ => {}
                    }
                    if version_pkt.is_some() && datetime_pkt.is_some() {
                        break;
                    }
                }
            }
            _ => break,
        }
    }

    // Print raw bytes for the protocol doc / capture file.
    for (idx, buf) in all_seen.iter().enumerate() {
        println!("packet {} raw:", idx + 1);
        for (i, b) in buf.iter().enumerate() {
            if i % 16 == 0 {
                print!("\n  {i:02}: ");
            }
            print!("{b:02x} ");
        }
        println!();
    }

    let version_pkt = version_pkt
        .ok_or_else(|| anyhow!("no cmd-134 packet with PacketNumber=0 (version) in response"))?;

    // Extract version string from the version packet header.
    let version_parsed = ACommandPacket::parse_response(&version_pkt, cmd_fw)
        .context("version packet header parse failed")?;
    let version_str = String::from_utf8_lossy(version_parsed.data()).to_string();

    // Datetime packet is optional on this firmware revision — observed
    // to arrive sporadically (sometimes immediately, sometimes only once
    // per power cycle). If we got it, use it; otherwise empty string.
    let datetime_str = match datetime_pkt {
        Some(buf) => {
            let p = ACommandPacket::parse_response(&buf, cmd_fw)
                .context("datetime packet header parse failed")?;
            String::from_utf8_lossy(p.data()).to_string()
        }
        None => {
            eprintln!("(no PacketNumber=1 datetime packet observed — unreliable on FW 1.6)");
            String::new()
        }
    };

    match FirmwareVersion::parse(&version_str, &datetime_str) {
        Ok(fw) => {
            println!();
            println!("decoded:");
            println!("  chip_code        = {}", fw.chip_code);
            println!("  boot_code        = {}", fw.boot_code);
            println!("  kind             = {}", fw.kind);
            println!("  model_a          = {}", fw.model_a);
            println!("  model_b          = {}", fw.model_b);
            println!("  software_version = {}", fw.software_version);
            println!("  user_version     = {}", fw.user_version);
            if fw.raw_datetime.is_empty() {
                println!("  raw_datetime     = (empty — datetime packet not observed)");
            } else {
                println!("  raw_datetime     = {}", fw.raw_datetime);
            }
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}\nraw bytes printed above")),
    }
}

fn fan(percent: u8) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::fan_pwm::build_request;
    use lconnect_protocol::galahad2_lcdv2::pwm::Pwm;

    let pwm = Pwm::new(percent).map_err(|e| anyhow!("invalid percent: {e}"))?;

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device")?;

    let req = build_request(pwm);
    let written = device.write(&req).context("HID write failed")?;
    println!("wrote {written} bytes (SetFanPWM, percent={percent})");
    println!("note: command is fire-and-forget; no response read.");
    println!("verify by running `verify-galahad handshake` to read fan_rpm.");
    Ok(())
}

fn pump(
    percent: u8,
    source: lconnect_protocol::galahad2_lcdv2::pump_pwm::PumpPwmSource,
) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::pump_pwm::build_request;
    use lconnect_protocol::galahad2_lcdv2::pwm::Pwm;

    let pwm = Pwm::new(percent).map_err(|e| anyhow!("invalid percent: {e}"))?;

    if percent < 30 {
        eprintln!(
            "WARNING: pump percent < 30 can cause the AIO to overheat. \
             Re-run with --percent >=30 unless you know what you're doing."
        );
    }

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device")?;

    let req = build_request(source, pwm);
    let written = device.write(&req).context("HID write failed")?;
    println!("wrote {written} bytes (SetPumpPWM, source={source:?}, percent={percent})");
    println!("note: command is fire-and-forget; no response read.");
    println!("verify by running `verify-galahad handshake` to read pump_rpm.");
    Ok(())
}

fn reset_device(yes: bool) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;
    use lconnect_protocol::galahad2_lcdv2::reset::{build_request, parse_response};

    if !yes {
        return Err(anyhow!(
            "ResetDevice (cmd 142) drops the AIO from USB for 1-3 seconds, \
             which stops the pump and can spike coolant temperature.\n\
             To run anyway, pass --yes-i-really-want-to-reset."
        ));
    }

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device")?;

    println!("sending ResetDevice...");
    let req = build_request();
    device.write(&req).context("HID write failed")?;

    let mut buf = [0u8; PACKET_LEN];
    let read = device
        .read_timeout(&mut buf, 2000)
        .context("HID read_timeout failed (device may have already dropped from USB)")?;
    println!("read {read} bytes");
    parse_response(&buf).context("reset response decode failed")?;
    println!("reset acknowledged. The device will now drop from USB; replug or wait.");
    Ok(())
}

fn get_pump_lighting() -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::commands::Command;
    use lconnect_protocol::galahad2_lcdv2::get_lighting::{
        build_get_pump_lighting_info_request, parse_pump_lighting_info_response,
    };
    use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    let req = build_get_pump_lighting_info_request();
    let written = device.write(&req).context("HID write failed")?;
    println!("wrote {written} bytes (GetPumpLightingInfo)");

    let buf: [u8; PACKET_LEN] =
        read_until_command(&device, Command::GetPumpLightingInfo as u8, 1000)?;
    println!("read {PACKET_LEN} bytes");

    match parse_pump_lighting_info_response(&buf) {
        Ok(info) => {
            println!();
            println!("decoded:");
            println!("  max_speed      = {}", info.max_speed);
            println!("  max_brightness = {}", info.max_brightness);
            println!("  modes ({}):", info.modes.len());
            for m in &info.modes {
                println!("    - {m:?} (0x{:02x})", *m as u8);
            }
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}")),
    }
}

fn set_pump_lighting(args: SetPumpLightingArgs) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::lighting::PumpLightingSetting;
    use lconnect_protocol::galahad2_lcdv2::set_lighting::build_set_pump_lighting_request;

    let brightness_wire = args.brightness / 25;
    let speed_wire = args.speed / 25;
    let setting = PumpLightingSetting {
        scope: args.scope.into(),
        mode: args.mode.into(),
        brightness: brightness_wire,
        speed: speed_wire,
        color1: args.color,
        direction: args.direction.into(),
        disabled: args.disable,
        source: args.source.into(),
        ..PumpLightingSetting::default()
    };

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device")?;

    let req = build_set_pump_lighting_request(setting);
    let written = device.write(&req).context("HID write failed")?;
    println!(
        "wrote {written} bytes (SetPumpLighting, mode={mode:?}, color=#{r:02X}{g:02X}{b:02X}, \
         brightness={brp}%->{brw} speed={spp}%->{spw} scope={sc:?}, source={src:?}, direction={dir:?}, disabled={dis})",
        mode = setting.mode,
        r = args.color.r,
        g = args.color.g,
        b = args.color.b,
        brp = args.brightness,
        brw = brightness_wire,
        spp = args.speed,
        spw = speed_wire,
        sc = setting.scope,
        src = setting.source,
        dir = setting.direction,
        dis = setting.disabled,
    );
    println!("note: command is fire-and-forget; LEDs may take ~50ms to update.");
    println!("verify by looking at the pump LEDs (the device does not echo the active setting).");
    Ok(())
}

fn get_fan_lighting() -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::commands::Command;
    use lconnect_protocol::galahad2_lcdv2::get_lighting::{
        build_get_fan_lighting_info_request, parse_fan_lighting_info_response,
    };
    use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    let req = build_get_fan_lighting_info_request();
    let written = device.write(&req).context("HID write failed")?;
    println!("wrote {written} bytes (GetFanLightingInfo)");

    let buf: [u8; PACKET_LEN] =
        read_until_command(&device, Command::GetFanLightingInfo as u8, 1000)?;
    println!("read {PACKET_LEN} bytes");

    match parse_fan_lighting_info_response(&buf) {
        Ok(info) => {
            println!();
            println!("decoded:");
            println!("  max_speed      = {}", info.max_speed);
            println!("  max_brightness = {}", info.max_brightness);
            println!("  modes ({}):", info.modes.len());
            for m in &info.modes {
                println!("    - {m:?} (0x{:02x})", *m as u8);
            }
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}")),
    }
}

fn set_fan_lighting(args: SetFanLightingArgs) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::lighting::FanLightingSetting;
    use lconnect_protocol::galahad2_lcdv2::set_lighting::build_set_fan_lighting_request;

    let brightness_wire = args.brightness / 25;
    let speed_wire = args.speed / 25;
    let setting = FanLightingSetting {
        mode: args.mode.into(),
        brightness: brightness_wire,
        speed: speed_wire,
        color1: args.color,
        direction: args.direction.into(),
        disabled: args.disable,
        source: args.source.into(),
        sync_to_pump: args.sync_to_pump,
        number_of_led: args.number_of_led,
        ..FanLightingSetting::default()
    };

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device")?;

    let req = build_set_fan_lighting_request(setting);
    let written = device.write(&req).context("HID write failed")?;
    println!(
        "wrote {written} bytes (SetFanLighting, mode={mode:?}, color=#{r:02X}{g:02X}{b:02X}, \
         brightness={brp}%->{brw} speed={spp}%->{spw} source={src:?}, direction={dir:?}, \
         disabled={dis}, sync_to_pump={stp}, number_of_led={nled})",
        mode = setting.mode,
        r = args.color.r,
        g = args.color.g,
        b = args.color.b,
        brp = args.brightness,
        brw = brightness_wire,
        spp = args.speed,
        spw = speed_wire,
        src = setting.source,
        dir = setting.direction,
        dis = setting.disabled,
        stp = setting.sync_to_pump,
        nled = setting.number_of_led,
    );
    println!("note: command is fire-and-forget; LEDs may take ~50ms to update.");
    println!("verify by looking at the fan LEDs (the device does not echo the active setting).");
    Ok(())
}

fn set_led_colors(args: SetLedColorsArgs) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::bpacket::{INPUT_LEN, OUTPUT_LEN};
    use lconnect_protocol::galahad2_lcdv2::lcd::{
        build_is_lcd_available_request, build_lcd_control_request, parse_is_lcd_available_response,
        Brightness, LCDControlMode, LCDControlSetting,
    };
    use lconnect_protocol::galahad2_lcdv2::set_led_colors::{
        build_set_fan_led_colors_request, build_set_screen_led_colors_request,
    };
    use lconnect_protocol::galahad2_lcdv2::Rgb;
    use std::io::Write as _;

    let kind_str = match args.kind {
        LedKindArg::Screen => "screen",
        LedKindArg::Fan => "fan",
    };

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    // Optional preflight: ask the device whether the LCD is available.
    // The IsLCDAvailable response is a 512-byte BCommandPacket (not a
    // 64-byte ACommandPacket), so we read INPUT_LEN bytes inline. The
    // ACommandPacket-style drain loop in `read_until_command` exists to
    // sidestep cmd-0x8e unsolicited A-class packets observed in Phase 0c;
    // those are A-class only, so a single inline read is correct here.
    if args.check_available {
        let req = build_is_lcd_available_request();
        let written = device
            .write(&req)
            .context("HID write failed (IsLCDAvailable)")?;
        println!("wrote {written} bytes (IsLCDAvailable preflight)");

        let buf = read_b_packet(&device, 1000)?;
        println!("read {INPUT_LEN} bytes");
        match parse_is_lcd_available_response(&buf) {
            Ok(true) => println!("LCD available — proceeding."),
            Ok(false) => {
                return Err(anyhow!(
                    "device reported LCD unavailable (IsLCDAvailable -> false). Try replugging \
                     the device or waiting a few seconds for the LCD to initialise, then re-run. \
                     If you want to bypass the check, omit --check-available — but a `false` \
                     report typically means the LCD is mid-init and a SetLEDColors write may not \
                     render."
                ));
            }
            Err(e) => {
                return Err(anyhow!(
                    "IsLCDAvailable response decode failed: {e}. \
                     Aborting before SetLEDColors."
                ));
            }
        }
    }

    // User-consent gate. The prompt MUST block on stdin and accept only
    // "y"/"Y"; arbitrary truthy strings are rejected.
    if !args.yes {
        print!(
            "WARNING: this will write {count} {kind} LEDs to #{r:02X}{g:02X}{b:02X} on the device.\n\
             The pump-cap LCD ring may visibly change. Continue? [y/N] ",
            count = args.count,
            kind = kind_str,
            r = args.color.r,
            g = args.color.g,
            b = args.color.b,
        );
        std::io::stdout().flush().ok();
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .context("failed to read confirmation from stdin")?;
        let trimmed = line.trim();
        if !trimmed.eq_ignore_ascii_case("y") {
            return Err(anyhow!(
                "aborted by user (got {trimmed:?}, expected \"y\" or \"Y\"). \
                 Re-run with --yes to skip this prompt."
            ));
        }
    }

    // Optional preflight: put the firmware into host-driven (Application)
    // mode. cmd 12 is fire-and-forget — no response is expected. This is
    // a state-changing write, so it MUST be gated behind the consent
    // prompt above — otherwise a user who types "N" at the prompt has
    // already had the device flipped to Application mode (LocalUI logo
    // gone dark) before the abort.
    //
    // `LCDControlSetting` is `#[non_exhaustive]`, so functional-update
    // (`..Self::default()`) is forbidden from outside the protocol crate.
    // Instead start from `Default` (gives Mode=LocalUI, Brightness=50,
    // Rotation=0, EnableTest=false, TestColor=Black, VideoFPS=30) and
    // override the mode in place.
    if args.set_application_mode {
        let mut setting = LCDControlSetting::default();
        setting.mode = LCDControlMode::Application;
        setting.brightness = Brightness::default();
        let req = build_lcd_control_request(setting);
        let written = device
            .write(&req)
            .context("HID write failed (LCDControl preflight — SetLEDColors was NOT sent)")?;
        println!("wrote {written} bytes (LCDControl, mode=Application, no response expected)");
    }

    // Build the colour array and request packets. The codec returns
    // `crate::Result<Vec<[u8; OUTPUT_LEN]>>`; `crate::Error` impls
    // `std::error::Error` (thiserror), so `?` lifts it into anyhow.
    let colors: Vec<Rgb> = vec![args.color; args.count as usize];
    let packets: Vec<[u8; OUTPUT_LEN]> = match args.kind {
        LedKindArg::Screen => build_set_screen_led_colors_request(&colors)
            .context("build_set_screen_led_colors_request failed")?,
        LedKindArg::Fan => build_set_fan_led_colors_request(&colors)
            .context("build_set_fan_led_colors_request failed")?,
    };

    let total = packets.len();
    for (i, packet) in packets.iter().enumerate() {
        let written = device.write(packet).with_context(|| {
            format!(
                "HID write failed on packet {} of {} (SetLEDColors)",
                i + 1,
                total
            )
        })?;
        if written != OUTPUT_LEN {
            return Err(anyhow!(
                "short HID write on packet {} of {}: sent {} of {} bytes (SetLEDColors). \
                 Device state may be partially updated; rerun the same command to retry.",
                i + 1,
                total,
                written,
                OUTPUT_LEN
            ));
        }
        if total > 1 {
            println!(
                "wrote {} bytes (SetLEDColors packet {}/{})",
                written,
                i + 1,
                total
            );
        }
    }
    println!("note: command is fire-and-forget; LEDs may take ~50ms to update.");
    println!(
        "verify by looking at the {kind_str} ring. If no visible change occurs, \
         retry with --set-application-mode (host-driven LCD mode may be required \
         for cmd 20 on some firmware revisions; this is empirically unverified)."
    );
    Ok(())
}

fn upload_jpeg(args: UploadJpegArgs) -> Result<()> {
    use lconnect_image::{prepare_jpeg_from_path, FitMode, Quality};
    use lconnect_protocol::galahad2_lcdv2::bpacket::{INPUT_LEN, OUTPUT_LEN};
    use lconnect_protocol::galahad2_lcdv2::jpeg::{
        build_send_jpeg_request, build_upload_jpeg_request,
    };
    use lconnect_protocol::galahad2_lcdv2::lcd::{
        build_is_lcd_available_request, build_lcd_control_request, parse_is_lcd_available_response,
        Brightness, LCDControlMode, LCDControlSetting,
    };
    use std::io::Write as _;

    // Refuse pathological inputs (e.g., uncompressed TIFF, decompression-bomb
    // PNG) before `image::open` decodes them into RAM. 50 MiB is several
    // orders of magnitude above any sane 480×480 source; well above this
    // and we're protecting the user from an OOM rather than serving a
    // legitimate use case.
    const MAX_INPUT_FILE_BYTES: u64 = 50 * 1024 * 1024; // 50 MiB

    let meta =
        std::fs::metadata(&args.path).with_context(|| format!("stat {}", args.path.display()))?;
    if meta.len() > MAX_INPUT_FILE_BYTES {
        return Err(anyhow!(
            "input file {} is {} bytes; refusing to load (max {} bytes / 50 MiB). \
             For larger images, downscale first with an external tool.",
            args.path.display(),
            meta.len(),
            MAX_INPUT_FILE_BYTES,
        ));
    }

    // 1. Convert input image. lconnect-image errors propagate via anyhow's
    //    `Context` (its `Error` impls `std::error::Error` via thiserror).
    let fit: FitMode = fit_mode_from_args(args.fit, args.letterbox_bg);
    let quality = Quality::new(args.quality)
        .with_context(|| format!("invalid --quality {}", args.quality))?;
    let prepared = prepare_jpeg_from_path(&args.path, fit, quality)
        .with_context(|| format!("reading or decoding input image {}", args.path.display()))?;
    println!(
        "prepared {} bytes of {}x{} JPEG (mode={}, fit={}, quality={})",
        prepared.bytes.len(),
        prepared.width,
        prepared.height,
        args.mode.as_str(),
        args.fit.as_str(),
        args.quality,
    );

    // 2. Open device.
    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    // 3. Optional IsLCDAvailable preflight (read-only; runs BEFORE consent).
    //    The IsLCDAvailable response is a 512-byte BCommandPacket, so
    //    `read_b_packet` is the right helper here (B-class reads don't
    //    need the cmd-0x8e drain loop that A-class reads use).
    if args.check_available {
        let req = build_is_lcd_available_request();
        let written = device
            .write(&req)
            .context("HID write failed (IsLCDAvailable)")?;
        println!("wrote {written} bytes (IsLCDAvailable preflight)");

        let buf = read_b_packet(&device, 1000)?;
        println!("read {INPUT_LEN} bytes");
        match parse_is_lcd_available_response(&buf) {
            Ok(true) => println!("LCD available — proceeding."),
            Ok(false) => {
                return Err(anyhow!(
                    "device reported LCD unavailable (IsLCDAvailable -> false). Try replugging \
                     the device or waiting a few seconds for the LCD to initialise, then re-run. \
                     If you want to bypass the check, omit --check-available — but a `false` \
                     report typically means the LCD is mid-init and a JPEG upload may not render."
                ));
            }
            Err(e) => {
                return Err(anyhow!(
                    "IsLCDAvailable response decode failed: {e}. \
                     Aborting before JPEG upload."
                ));
            }
        }
    }

    // 4. Build packets via the chosen codec. This is pure (no I/O, no
    //    device side effects) so we do it BEFORE the consent prompt — that
    //    way the prompt can quote the actual packet count and the JPEG
    //    payload size, giving the user authoritative numbers to consent
    //    to. Both codecs produce 1024-byte BCommandPacket frames; only
    //    the cmd byte differs.
    let packets: Vec<[u8; OUTPUT_LEN]> = match args.mode {
        JpegModeArg::Send => {
            build_send_jpeg_request(&prepared.bytes).context("building SendJpeg request")?
        }
        JpegModeArg::Upload => {
            build_upload_jpeg_request(&prepared.bytes).context("building UploadJpeg request")?
        }
    };
    let total = packets.len();

    // 5. User-consent gate (skippable with --yes). Blocks on stdin and
    //    accepts only "y"/"Y" (case-insensitive); arbitrary truthy
    //    strings are rejected. The prompt body differentiates by mode:
    //    cmd 14 (Send) is the standard "this will change the LCD"
    //    warning; cmd 15 (Upload) prepends a flash-wear warning because
    //    UploadJpeg is presumed to write to non-volatile storage.
    if !args.yes {
        let plural = if total == 1 { "" } else { "s" };
        let warning = match args.mode {
            JpegModeArg::Send => format!(
                "WARNING: this will write {total} packet{plural} ({jpeg_size}-byte JPEG \
                 via cmd 14, SendJpeg) to the LCD on device {vid:04x}:{pid:04x}. The \
                 pump-cap LCD content will visibly change. Continue? [y/N] ",
                total = total,
                plural = plural,
                jpeg_size = prepared.bytes.len(),
                vid = GALAHAD2_LCDV2_VID,
                pid = GALAHAD2_LCDV2_PID,
            ),
            JpegModeArg::Upload => format!(
                "WARNING: cmd 15 (UploadJpeg) may write to non-volatile flash storage. \
                 Flash cells have a finite write-cycle life (~10K-100K). Do not run in \
                 loops. This will write {total} packet{plural} ({jpeg_size}-byte JPEG) \
                 to the LCD on device {vid:04x}:{pid:04x}. Continue? [y/N] ",
                total = total,
                plural = plural,
                jpeg_size = prepared.bytes.len(),
                vid = GALAHAD2_LCDV2_VID,
                pid = GALAHAD2_LCDV2_PID,
            ),
        };
        print!("{}", warning);
        std::io::stdout()
            .flush()
            .context("failed to flush stdout before consent prompt")?;
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .context("failed to read confirmation from stdin")?;
        let trimmed = line.trim();
        if !trimmed.eq_ignore_ascii_case("y") {
            return Err(anyhow!(
                "aborted by user (got {trimmed:?}, expected \"y\" or \"Y\"). \
                 Re-run with --yes to skip this prompt."
            ));
        }
    }

    // 6. Optional LCDControl{mode=Application} preflight. cmd 12 is
    //    fire-and-forget. This is a state-changing write, so it runs
    //    AFTER the consent prompt — same rule as set-led-colors. Use
    //    let-mut + field assign because `LCDControlSetting` is
    //    `#[non_exhaustive]` from outside the protocol crate, which
    //    forbids functional-update syntax.
    if args.set_application_mode {
        let mut setting = LCDControlSetting::default();
        setting.mode = LCDControlMode::Application;
        setting.brightness = Brightness::default();
        let req = build_lcd_control_request(setting);
        let written = device
            .write(&req)
            .context("HID write failed (LCDControl preflight — JPEG upload was NOT sent)")?;
        println!("wrote {written} bytes (LCDControl, mode=Application, no response expected)");
    }

    // 7. Per-packet write loop with short-write detection and per-failure
    //    context. Suppresses progress lines when total == 1.
    for (i, packet) in packets.iter().enumerate() {
        let mode_str = args.mode.as_str();
        let written = device.write(packet).with_context(|| {
            format!(
                "HID write failed on packet {} of {} ({}). \
                 If retrying does not recover the LCD, run: \
                 verify-galahad reset --yes-i-really-want-to-reset",
                i + 1,
                total,
                mode_str,
            )
        })?;
        if written != OUTPUT_LEN {
            return Err(anyhow!(
                "short HID write on packet {} of {}: sent {} of {} bytes ({}). \
                 Device state may be partially updated; rerun the same command to retry. \
                 If retrying does not recover the LCD, run: \
                 verify-galahad reset --yes-i-really-want-to-reset",
                i + 1,
                total,
                written,
                OUTPUT_LEN,
                mode_str,
            ));
        }
        if total > 1 {
            println!(
                "wrote {} bytes ({} packet {}/{})",
                written,
                mode_str,
                i + 1,
                total,
            );
        }
    }

    // 8. Success message.
    println!("note: command is fire-and-forget; LCD frame may take ~50-200ms to update.");
    println!(
        "verify by looking at the pump-cap LCD. If no visible change occurs, retry \
         with --set-application-mode (host-driven LCD mode may be required for \
         cmd 14/15 on some firmware revisions; this is empirically unverified)."
    );
    if args.set_application_mode {
        println!(
            "note: --set-application-mode leaves the LCD in host-driven Application mode. \
             The device stays in this mode until replugged, reset (cmd 142), or sent another \
             LCDControl with mode=LocalUI."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cmd_byte_arg_decimal() {
        assert_eq!(parse_cmd_byte_arg("134"), Ok(134));
    }

    #[test]
    fn parse_cmd_byte_arg_hex_lowercase() {
        assert_eq!(parse_cmd_byte_arg("0x86"), Ok(0x86));
    }

    #[test]
    fn parse_cmd_byte_arg_hex_uppercase() {
        assert_eq!(parse_cmd_byte_arg("0X86"), Ok(0x86));
    }

    #[test]
    fn parse_cmd_byte_arg_with_whitespace() {
        // The fn calls .trim() on the input
        assert_eq!(parse_cmd_byte_arg("  134  "), Ok(134));
    }

    #[test]
    fn parse_cmd_byte_arg_rejects_overflow_decimal() {
        assert!(parse_cmd_byte_arg("256").is_err());
    }

    #[test]
    fn parse_cmd_byte_arg_rejects_overflow_hex() {
        assert!(parse_cmd_byte_arg("0x100").is_err());
    }

    #[test]
    fn parse_cmd_byte_arg_rejects_non_hex() {
        assert!(parse_cmd_byte_arg("0xZZ").is_err());
    }
}
