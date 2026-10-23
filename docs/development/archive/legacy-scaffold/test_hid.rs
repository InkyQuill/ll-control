// Simple test to enumerate HID devices
use hidapi::HidApi;

fn main() -> anyhow::Result<()> {
    println!("=== LL-Control HID Test ===\n");

    let api = HidApi::new()?;

    println!("Enumerating all HID devices...\n");

    for device in api.device_list() {
        let vid = device.vendor_id();
        let pid = device.product_id();

        // Print all devices
        println!(
            "Device: {:04x}:{:04x} - {}",
            vid,
            pid,
            device.product_string().unwrap_or("Unknown")
        );

        // Check if it's our GALAHAD II
        if vid == 0x416 && pid == 0x7381 {
            println!("  *** THIS IS GALAHAD II! ***");
        }
    }

    // Try to find GALAHAD II specifically
    println!("\n=== Looking for GALAHAD II (0416:7395) ===");

    let galahad = api
        .device_list()
        .find(|d| d.vendor_id() == 0x416 && d.product_id() == 0x7395);

    if let Some(device) = galahad {
        println!("\n✓ GALAHAD II found!");
        println!("  Path: {:?}", device.path());
        println!(
            "  Manufacturer: {}",
            device.manufacturer_string().unwrap_or("Unknown")
        );
        println!(
            "  Product: {}",
            device.product_string().unwrap_or("Unknown")
        );
        println!("  Interface: {}", device.interface_number());

        // Try to open it
        println!("\n=== Attempting to open device ===");
        match api.open(0x416, 0x7395) {
            Ok(device) => {
                println!("✓ Device opened successfully!");

                // Send handshake command
                println!("\n=== Sending handshake command (129) ===");
                let mut packet = [0u8; 64];
                packet[0] = 1; // Report ID
                packet[1] = 129; // GetHandshakeInfo command

                match device.write(&packet) {
                    Ok(_) => println!("✓ Command sent"),
                    Err(e) => println!("✗ Failed to send: {}", e),
                }

                // Read response
                println!("\n=== Reading response ===");
                let mut buf = [0u8; 64];
                match device.read_timeout(&mut buf, 1000) {
                    Ok(bytes_read) => {
                        println!("✓ Received {} bytes", bytes_read);

                        if bytes_read >= 7 {
                            let fan_rpm = u16::from_le_bytes([buf[0], buf[1]]);
                            let pump_rpm = u16::from_le_bytes([buf[2], buf[3]]);
                            let temp_valid = buf[4] != 0;
                            let temp_integer = buf[5];
                            let temp_fraction = buf[6];

                            println!("\n=== Handshake Data ===");
                            println!("  Fan RPM:   {}", fan_rpm);
                            println!("  Pump RPM:  {}", pump_rpm);

                            if temp_valid {
                                let temp = temp_integer as f32 + (temp_fraction as f32 / 10.0);
                                println!("  Temperature: {:.1}°C", temp);
                            } else {
                                println!("  Temperature: N/A (invalid)");
                            }
                        }
                    }
                    Err(e) => println!("✗ Failed to read: {}", e),
                }
            }
            Err(e) => println!("✗ Failed to open device: {}", e),
        }
    } else {
        println!("\n✗ GALAHAD II not found!");
        println!("  Make sure:");
        println!("  - Device is connected via USB");
        println!("  - Device is powered on");
        println!("  - You have permission to access HID devices");
        println!("\n  Try running: sudo ./target/debug/test_hid");
    }

    Ok(())
}
