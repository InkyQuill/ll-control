# Setup Instructions for LL-Control

## 1. Install udev rule for device permissions

Copy the udev rule to system directory:

```bash
sudo cp 99-lian-li-galahad2.rules /etc/udev/rules.d/
```

## 2. Reload udev rules

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## 3. Reconnect device

Unplug and reconnect your GALAHAD II device, or run:

```bash
sudo udevadm trigger /dev/hidraw2
```

## 4. Test permissions

```bash
# Check if device is accessible
ls -l /dev/hidraw2

# Should show crw-rw-rw- (0666)
```

## Alternative: Add user to plugdev group

```bash
# Add current user to plugdev group
sudo usermod -a -G plugdev $USER

# Log out and log back in for changes to take effect
```

## Quick Test

After setting up permissions, run:

```bash
./target/release/test_hid
```

You should see:
- ✓ GALAHAD II found!
- ✓ Device opened successfully!
- ✓ Handshake data (Fan RPM, Pump RPM, Temperature)

## Run GUI Application

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized)
cargo build --release

# Run
./target/release/lconnect_linux
```
