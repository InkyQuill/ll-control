// Advanced HID test - try different packet formats
use hidapi::HidApi;
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    println!("=== Advanced L-Connect HID Test ===\n");

    let api = HidApi::new()?;
    let device = api.open(0x416, 0x7395)?;

    device.set_blocking_mode(false)?;

    println!("Device opened. Testing different packet formats...\n");

    // Test 1: Original format (64 bytes, Report ID = 1, Command = 129)
    println!("=== Test 1: Standard ACommandPacket ===");
    let mut packet1 = [0u8; 64];
    packet1[0] = 1;   // Report ID
    packet1[1] = 129; // Command: GetHandshakeInfo

    println!("Sending: {:?}", &packet1[..7]);
    device.write(&packet1)?;

    // Try different timeouts
    for timeout in [100, 500, 1000, 2000] {
        thread::sleep(Duration::from_millis(100));
        let mut buf = [0u8; 64];
        match device.read_timeout(&mut buf, timeout) {
            Ok(n) if n > 0 => {
                println!("✓ Received {} bytes after {}ms timeout", n, timeout);
                print_hex(&buf, n);
                break;
            }
            Ok(_) => {}
            Err(e) => {
                println!("✗ Error with {}ms timeout: {}", timeout, e);
            }
        }
    }

    // Test 2: Try without Report ID (just command)
    println!("\n=== Test 2: Command only (no Report ID) ===");
    let mut packet2 = [0u8; 64];
    packet2[0] = 129; // Command at position 0

    println!("Sending: {:?}", &packet2[..7]);
    device.write(&packet2)?;

    thread::sleep(Duration::from_millis(500));
    let mut buf = [0u8; 64];
    match device.read_timeout(&mut buf, 2000) {
        Ok(n) if n > 0 => {
            println!("✓ Received {} bytes", n);
            print_hex(&buf, n);
        }
        Ok(_) => println!("✗ No data received"),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Test 3: Try Feature Report (SendFeatureReport + GetFeatureReport)
    println!("\n=== Test 3: Feature Report ===");

    // Send Feature Report
    let mut packet3 = [0u8; 65]; // Feature reports need extra byte
    packet3[0] = 0;   // Report ID for feature (usually 0)
    packet3[1] = 1;   // Report ID for data
    packet3[2] = 129; // Command

    println!("Sending Feature Report: {:?}", &packet3[..10]);
    match device.send_feature_report(&packet3) {
        Ok(_) => println!("✓ Feature report sent"),
        Err(e) => println!("✗ Failed to send feature report: {}", e),
    }

    thread::sleep(Duration::from_millis(500));

    // Get Feature Report
    let mut buf = [0u8; 65];
    match device.get_feature_report(&mut buf) {
        Ok(n) => {
            println!("✓ Received {} bytes via feature report", n);
            print_hex(&buf, n);
        }
        Err(e) => println!("✗ Failed to get feature report: {}", e),
    }

    // Test 4: Try different command codes
    println!("\n=== Test 4: Try command 23 (IsLCDAvailable) ===");
    let mut packet4 = [0u8; 64];
    packet4[0] = 1;
    packet4[1] = 23; // IsLCDAvailable

    println!("Sending: {:?}", &packet4[..7]);
    device.write(&packet4)?;

    thread::sleep(Duration::from_millis(500));
    let mut buf = [0u8; 64];
    match device.read_timeout(&mut buf, 2000) {
        Ok(n) if n > 0 => {
            println!("✓ Received {} bytes", n);
            print_hex(&buf, n);
        }
        Ok(_) => println!("✗ No data received"),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Test 5: Try firmware version command (134)
    println!("\n=== Test 5: Try command 134 (GetFirmwareVersion) ===");
    let mut packet5 = [0u8; 64];
    packet5[0] = 1;
    packet5[1] = 134; // GetFirmwareVersion

    println!("Sending: {:?}", &packet5[..7]);
    device.write(&packet5)?;

    thread::sleep(Duration::from_millis(500));
    let mut buf = [0u8; 64];
    match device.read_timeout(&mut buf, 2000) {
        Ok(n) if n > 0 => {
            println!("✓ Received {} bytes", n);
            print_hex(&buf, n);
        }
        Ok(_) => println!("✗ No data received"),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Test 6: Try reading without sending anything
    println!("\n=== Test 6: Read without command (check for spontaneous data) ===");
    for i in 1..=5 {
        thread::sleep(Duration::from_millis(200));
        let mut buf = [0u8; 64];
        match device.read_timeout(&mut buf, 500) {
            Ok(n) if n > 0 => {
                println!("✓ Received {} bytes on attempt {}", n, i);
                print_hex(&buf, n);
                break;
            }
            Ok(_) => {}
            Err(e) => println!("✗ Error on attempt {}: {}", i, e),
        }
    }

    println!("\n=== Testing complete ===");

    Ok(())
}

fn print_hex(data: &[u8], len: usize) {
    for chunk in data[..len].chunks(16) {
        print!("    ");
        for (i, byte) in chunk.iter().enumerate() {
            print!("{:02x} ", byte);
            if i == 7 {
                print!(" ");
            }
        }
        println!();

        // ASCII representation
        print!("    ");
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
}
