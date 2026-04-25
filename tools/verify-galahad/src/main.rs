//! Phase-0 test harness for Galahad II LCDV2.
//!
//! Not a product binary. This exists only to exercise commands defined
//! in `lconnect-protocol` against a live device and print the results
//! into `docs/protocol/galahad2-lcdv2.md`'s "Verification" sections.
//!
//! Once Phase 1 stands up the real daemon, this tool is retired.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use hidapi::HidApi;

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
}

/// Flags for `set-pump-lighting`.
#[derive(Args, Debug)]
struct SetPumpLightingArgs {
    /// Lighting mode (effect). Defaults to a solid colour.
    #[arg(long, default_value = "static-color")]
    mode: PumpModeArg,
    /// Primary colour as `#RRGGBB` (six hex digits, optional leading `#`).
    #[arg(long, default_value = "#FF0000")]
    color: String,
    /// Brightness, 0..=100.
    #[arg(long, default_value_t = 50)]
    brightness: u8,
    /// Animation speed, 0..=100. Use 0 for non-animated modes.
    #[arg(long, default_value_t = 0)]
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

/// Flags for `set-fan-lighting`.
#[derive(Args, Debug)]
struct SetFanLightingArgs {
    /// Lighting mode (effect). Defaults to a solid colour.
    #[arg(long, default_value = "static-color")]
    mode: FanModeArg,
    /// Primary colour as `#RRGGBB` (six hex digits, optional leading `#`).
    #[arg(long, default_value = "#FF0000")]
    color: String,
    /// Brightness, 0..=100.
    #[arg(long, default_value_t = 50)]
    brightness: u8,
    /// Animation speed, 0..=100. Use 0 for non-animated modes.
    #[arg(long, default_value_t = 0)]
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
    /// Number of LEDs in the fan ring (varies by SKU).
    #[arg(long, default_value_t = 24)]
    number_of_led: u8,
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

    // Read up to 64 bytes of response with a 1 s timeout.
    let mut buf = [0u8; PACKET_LEN];
    let read = device
        .read_timeout(&mut buf, 1000)
        .context("HID read_timeout failed")?;
    println!("read {} bytes", read);

    // Print the raw bytes for the protocol doc.
    print!("raw response:");
    for (i, b) in buf.iter().enumerate().take(read.max(13)) {
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

    let mut buf = [0u8; PACKET_LEN];
    let read = device
        .read_timeout(&mut buf, 1000)
        .context("HID read_timeout failed")?;
    println!("read {read} bytes");

    match parse_pump_lighting_info_response(&buf) {
        Ok(info) => {
            println!();
            println!("decoded:");
            println!("  max_speed      = {}", info.max_speed);
            println!("  max_brightness = {}", info.max_brightness);
            println!("  modes ({}):", info.modes.len());
            for m in &info.modes {
                println!("    - {m:?}");
            }
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}")),
    }
}

fn set_pump_lighting(args: SetPumpLightingArgs) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::lighting::PumpLightingSetting;
    use lconnect_protocol::galahad2_lcdv2::set_lighting::build_set_pump_lighting_request;

    let color = parse_hex_color(&args.color)?;
    let setting = PumpLightingSetting {
        scope: args.scope.into(),
        mode: args.mode.into(),
        brightness: args.brightness,
        speed: args.speed,
        color1: color,
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
         brightness={br}, speed={sp}, scope={sc:?}, source={src:?}, direction={dir:?}, disabled={dis})",
        mode = setting.mode,
        r = color.r,
        g = color.g,
        b = color.b,
        br = setting.brightness,
        sp = setting.speed,
        sc = setting.scope,
        src = setting.source,
        dir = setting.direction,
        dis = setting.disabled,
    );
    println!("note: command is fire-and-forget; LEDs may take ~50ms to update.");
    println!("verify by looking at the pump LEDs, or run `verify-galahad get-pump-lighting` to read modes back.");
    Ok(())
}

fn get_fan_lighting() -> Result<()> {
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

    let mut buf = [0u8; PACKET_LEN];
    let read = device
        .read_timeout(&mut buf, 1000)
        .context("HID read_timeout failed")?;
    println!("read {read} bytes");

    match parse_fan_lighting_info_response(&buf) {
        Ok(info) => {
            println!();
            println!("decoded:");
            println!("  max_speed      = {}", info.max_speed);
            println!("  max_brightness = {}", info.max_brightness);
            println!("  modes ({}):", info.modes.len());
            for m in &info.modes {
                println!("    - {m:?}");
            }
            Ok(())
        }
        Err(e) => Err(anyhow!("decode failed: {e}")),
    }
}

fn set_fan_lighting(args: SetFanLightingArgs) -> Result<()> {
    use lconnect_protocol::galahad2_lcdv2::lighting::FanLightingSetting;
    use lconnect_protocol::galahad2_lcdv2::set_lighting::build_set_fan_lighting_request;

    let color = parse_hex_color(&args.color)?;
    let setting = FanLightingSetting {
        mode: args.mode.into(),
        brightness: args.brightness,
        speed: args.speed,
        color1: color,
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
         brightness={br}, speed={sp}, source={src:?}, direction={dir:?}, disabled={dis}, \
         sync_to_pump={stp}, number_of_led={nled})",
        mode = setting.mode,
        r = color.r,
        g = color.g,
        b = color.b,
        br = setting.brightness,
        sp = setting.speed,
        src = setting.source,
        dir = setting.direction,
        dis = setting.disabled,
        stp = setting.sync_to_pump,
        nled = setting.number_of_led,
    );
    println!("note: command is fire-and-forget; LEDs may take ~50ms to update.");
    println!("verify by looking at the fan LEDs, or run `verify-galahad get-fan-lighting` to read modes back.");
    Ok(())
}
