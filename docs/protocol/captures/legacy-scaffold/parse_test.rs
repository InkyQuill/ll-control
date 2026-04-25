// Detailed packet analysis - understand the byte layout
use hidapi::HidApi;

fn main() -> anyhow::Result<()> {
    println!("=== GALAHAD II Packet Analysis ===\n");

    let api = HidApi::new()?;
    let device = api.open(0x416, 0x7395)?;
    device.set_blocking_mode(false)?;

    // Send handshake command
    let mut packet = [0u8; 64];
    packet[0] = 1;   // Report ID
    packet[1] = 129; // Command

    device.write(&packet)?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Read response
    let mut buf = [0u8; 64];
    let bytes_read = device.read_timeout(&mut buf, 1000)?;

    println!("Raw response ({} bytes):", bytes_read);
    println!();

    // Print hex dump
    println!("Hex dump:");
    for (i, chunk) in buf.chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        println!();
    }
    println!();

    // Analysis based on C# code
    println!("=== ACommandPacket Structure ===");
    println!("[0] Report ID:     0x{:02x} ({})", buf[0], buf[0]);
    println!("[1] Command:       0x{:02x} ({})", buf[1], buf[1]);
    println!("[2-5] Header:      {:02x} {:02x} {:02x} {:02x}", buf[2], buf[3], buf[4], buf[5]);
    println!();

    // ACommandPacket.Data = bytes[6..64]
    println!("=== Payload (bytes 6-63) ===");
    println!("According to C# code, HandshakeInfo uses data[0..7] of payload");
    println!();

    // Correct parsing based on C#
    let fan_rpm_hi = buf[6];
    let fan_rpm_lo = buf[7];
    let pump_rpm_hi = buf[8];
    let pump_rpm_lo = buf[9];
    let temp_valid = buf[10];
    let temp_int = buf[11];
    let temp_frac = buf[12];

    let fan_rpm = u16::from_be_bytes([buf[6], buf[7]]);
    let pump_rpm = u16::from_be_bytes([buf[8], buf[9]]);

    println!("Parsing according to C# HandshakeInfo:");
    println!("  Fan RPM:    0x{:02x}{:02x} = {} RPM", fan_rpm_hi, fan_rpm_lo, fan_rpm);
    println!("  Pump RPM:   0x{:02x}{:02x} = {} RPM", pump_rpm_hi, pump_rpm_lo, pump_rpm);
    println!("  Temp Valid: {}", temp_valid != 0);
    println!("  Temp:       {} + {}/10 = {:.1}°C", temp_int, temp_frac, temp_int as f32 + (temp_frac as f32 / 10.0));
    println!();

    // Alternative: try little-endian for comparison
    let fan_rpm_le = u16::from_le_bytes([buf[6], buf[7]]);
    let pump_rpm_le = u16::from_le_bytes([buf[8], buf[9]]);

    println!("Alternative (little-endian):");
    println!("  Fan RPM:    0x{:02x}{:02x} = {} RPM", fan_rpm_hi, fan_rpm_lo, fan_rpm_le);
    println!("  Pump RPM:   0x{:02x}{:02x} = {} RPM", pump_rpm_hi, pump_rpm_lo, pump_rpm_le);
    println!();

    // What I was using before
    println!("What I was parsing (WRONG):");
    let fan_rpm_wrong = u16::from_le_bytes([buf[10], buf[11]]);
    let pump_rpm_wrong = u16::from_le_bytes([buf[12], buf[13]]);
    let temp_wrong = buf[14] as f32;
    println!("  Fan RPM:    {} (from bytes [10-11])", fan_rpm_wrong);
    println!("  Pump RPM:   {} (from bytes [12-13])", pump_rpm_wrong);
    println!("  Temp:       {:.1}°C (from byte [14])", temp_wrong);
    println!();

    Ok(())
}
