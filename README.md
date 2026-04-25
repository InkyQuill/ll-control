# L-Connect Linux

Linux client for Lian Li GALAHAD II LCD control.

## Features

- ✅ Connect to GALAHAD II devices via HID
- ✅ Read sensor data (temperature, fan/pump RPM)
- ✅ Control fan and pump PWM
- 🚧 Fan curve editor
- 🚧 LCD image display
- 🚧 LED lighting control

## Building

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install build-essential libudev-dev pkg-config

# Build
cargo build --release

# Run
./target/release/lconnect_linux
```

## Usage

1. Connect your GALAHAD II device
2. Run the application
3. Click "Connect to Device"
4. Adjust fan/pump PWM sliders
5. Click "Apply" to send changes

## Development

```bash
# Run with logging
RUST_LOG=debug ./target/release/lconnect_linux
```

## TODO

- [ ] Implement fan curve editor with drag-and-drop
- [ ] Add lm-sensors integration for CPU/GPU temp
- [ ] Implement JPEG upload to LCD
- [ ] Add LED color control
- [ ] Profile save/load
- [ ] System tray icon
