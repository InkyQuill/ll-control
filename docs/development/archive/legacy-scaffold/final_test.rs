// Final demonstration test - shows real device data
use hidapi::HidApi;

fn main() -> anyhow::Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║      L-Connect Linux - GALAHAD II LCD Test                 ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    let api = HidApi::new()?;

    // Check for device
    let device_info = api
        .device_list()
        .find(|d| d.vendor_id() == 0x416 && d.product_id() == 0x7395);

    if device_info.is_none() {
        println!("❌ GALAHAD II device not found!");
        println!("   Make sure:");
        println!("   • Device is connected via USB");
        println!("   • Device has power");
        println!("   • Udev rules are installed");
        return Ok(());
    }

    let device_info = device_info.unwrap();
    println!("✅ Device found: {}", device_info.product_string().unwrap_or("Unknown"));
    println!("   Path: {:?}", device_info.path());
    println!();

    // Open device
    let device = api.open(0x416, 0x7395)?;
    device.set_blocking_mode(false)?;

    println!("✅ Device opened successfully!");
    println!();

    // Get firmware version
    println!("─────────────────────────────────────────────────────────────");
    println!("📋 Firmware Information");
    println!("─────────────────────────────────────────────────────────────");

    let mut fw_packet = [0u8; 64];
    fw_packet[0] = 1;
    fw_packet[1] = 134; // GetFirmwareVersion

    device.write(&fw_packet)?;

    std::thread::sleep(std::time::Duration::from_millis(500));

    let mut buf = [0u8; 64];
    if device.read_timeout(&mut buf, 1000)? > 20 {
        let fw_bytes = &buf[6..];
        let null_end = fw_bytes.iter().position(|&b| b == 0).unwrap_or(fw_bytes.len());
        let fw_string = String::from_utf8_lossy(&fw_bytes[..null_end]);

        println!("   Version: {}", fw_string);
        println!();
    }

    // Read sensor data
    println!("─────────────────────────────────────────────────────────────");
    println!("🌡️  Real-time Sensor Data");
    println!("─────────────────────────────────────────────────────────────");
    println!();

    // Read 5 times with 1 second interval
    for i in 1..=5 {
        let mut packet = [0u8; 64];
        packet[0] = 1;
        packet[1] = 129; // GetHandshakeInfo

        device.write(&packet)?;

        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut buf = [0u8; 64];
        let bytes_read = device.read_timeout(&mut buf, 1000)?;

        if bytes_read >= 13 {
            // Correct parsing based on C# code
            let fan_rpm = u16::from_be_bytes([buf[6], buf[7]]);
            let pump_rpm = u16::from_be_bytes([buf[8], buf[9]]);
            let temp_valid = buf[10] != 0;
            let temp_int = buf[11];
            let temp_frac = buf[12];
            let temp = if temp_valid {
                temp_int as f32 + (temp_frac as f32 / 10.0)
            } else {
                0.0
            };

            print!("   Reading #{}: ", i);

            // Color code the output
            if temp > 60.0 {
                print!("🔴 ");
            } else if temp > 45.0 {
                print!("🟡 ");
            } else {
                print!("🟢 ");
            }

            println!(
                "Temp: {:>5.1}°C  |  Fan: {:>4} RPM  |  Pump: {:>4} RPM",
                temp, fan_rpm, pump_rpm
            );
        }

        if i < 5 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    println!();
    println!("─────────────────────────────────────────────────────────────");
    println!("✅ Test completed successfully!");
    println!("─────────────────────────────────────────────────────────────");
    println!();
    println!("Next steps:");
    println!("  • Run GUI:  ./target/release/lconnect_linux");
    println!("  • Control fans/pump with sliders");
    println!("  • Try sending images to LCD");
    println!();

    Ok(())
}
