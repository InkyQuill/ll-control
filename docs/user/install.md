# Install and start

This is a **software preview**, not yet a hardware-qualified release. The current
package targets Arch Linux x86_64; the first supported desktop target is Plasma
Wayland. Other distributions and devices are not yet qualified.

1. Obtain a package built for your system. There is no release-download promise
   here yet; developers can use the [package builder](../development/parity/package-delivery.md).
2. Stop any existing L-Connect daemon before upgrading:
   `systemctl --user stop lconnectd`.
3. Install the package: `sudo pacman -U ./lconnect-linux-<version>-x86_64.pkg.tar.zst`
   (replace the filename with the actual package).
4. Run `systemctl --user daemon-reload`, then
   `systemctl --user enable --now lconnectd`.
5. Open **L-Connect Linux** from your application menu.

Starting the daemon can restore saved settings to connected hardware. Do not run
another L-Connect daemon or a low-level device tool at the same time.

The package includes device permissions and the background service. If the device
is not accessible after installation, see [troubleshooting](troubleshooting.md).
Mini adapters are included but experimental; adding a widget or enabling an
extension is your choice. Installation does not alter your panels.

To upgrade, stop the service, install the new package, reload systemd and start
it again. To remove: `systemctl --user disable --now lconnectd`, then
`sudo pacman -R lconnect-linux`. Saved user data is retained.
