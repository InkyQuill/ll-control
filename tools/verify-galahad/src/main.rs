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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::List => list_devices(),
        Cmd::Handshake => handshake(),
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
