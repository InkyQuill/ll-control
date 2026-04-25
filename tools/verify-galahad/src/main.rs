//! Phase-0 test harness for Galahad II LCDV2.
//!
//! Not a product binary. This exists only to exercise commands defined
//! in `lconnect-protocol` against a live device and print the results
//! into `docs/protocol/galahad2-lcdv2.md`'s "Verification" sections.
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

    /// Number of LEDs to address. Default 8. Capped at 80 (well below
    /// `MAX_SCREEN_LEDS=329` / `MAX_FAN_LEDS=337`) to prevent typo-driven
    /// over-writes on small hardware rings — Galahad II LCDV2 ships with
    /// ~16-24 LEDs per ring.
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u8).range(1..=80))]
    count: u8,

    /// Which LED ring to target.
    #[arg(long, default_value = "screen")]
    kind: LedKindArg,

    /// Skip the "this will overwrite LCD content" prompt.
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
    }
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

        let mut buf = [0u8; INPUT_LEN];
        let read = device
            .read_timeout(&mut buf, 1000)
            .context("HID read_timeout failed (IsLCDAvailable response)")?;
        println!("read {read} bytes");
        match parse_is_lcd_available_response(&buf) {
            Ok(true) => println!("LCD available — proceeding."),
            Ok(false) => {
                return Err(anyhow!(
                    "device reported LCD unavailable (IsLCDAvailable -> false). \
                     Aborting before SetLEDColors. Re-run without --check-available \
                     if you want to send the write anyway."
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

    // Optional preflight: put the firmware into host-driven (Application)
    // mode. cmd 12 is fire-and-forget — no response is expected.
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
            .context("HID write failed (LCDControl)")?;
        println!("wrote {written} bytes (LCDControl, mode=Application, no response expected)");
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
        let written = device
            .write(packet)
            .context("HID write failed (SetLEDColors)")?;
        println!(
            "wrote {written} bytes (SetLEDColors packet {i_one}/{total}, kind={kind}, count={count}, color=#{r:02X}{g:02X}{b:02X})",
            i_one = i + 1,
            kind = kind_str,
            count = args.count,
            r = args.color.r,
            g = args.color.g,
            b = args.color.b,
        );
    }
    println!(
        "note: command is fire-and-forget; LEDs may take ~50ms to update. \
         Verify by looking at the {kind_str} ring."
    );
    Ok(())
}
