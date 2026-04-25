//! Phase-0 test harness for Galahad II LCDV2.
//!
//! Not a product binary. This exists only to exercise commands defined
//! in `lconnect-protocol` against a live device and print the results
//! into `docs/protocol/galahad2-lcdv2.md`'s "Verification" sections.
//!
//! Once Phase 1 stands up the real daemon, this tool is retired.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::List => list_devices(),
        Cmd::Handshake => handshake(),
        Cmd::Firmware => firmware(),
    }
}

fn list_devices() -> Result<()> {
    let api = HidApi::new().context("HidApi::new failed")?;
    let mut found = 0usize;
    for info in api.device_list() {
        if info.vendor_id() == GALAHAD2_LCDV2_VID
            && info.product_id() == GALAHAD2_LCDV2_PID
        {
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
    use lconnect_protocol::galahad2_lcdv2::handshake::{
        build_request, HandshakeInfo,
    };
    use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

    let api = HidApi::new().context("HidApi::new failed")?;
    let device = api
        .open(GALAHAD2_LCDV2_VID, GALAHAD2_LCDV2_PID)
        .context("could not open device — see `verify-galahad list` and udev rules")?;

    // Send the 64-byte request.
    let req = build_request();
    let written = device
        .write(&req)
        .context("HID write failed")?;
    println!("wrote {} bytes (expect 65 incl. report id, or 64 depending on backend)", written);

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
    use lconnect_protocol::galahad2_lcdv2::firmware::FirmwareVersion;
    use lconnect_protocol::galahad2_lcdv2::commands::Command;
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
