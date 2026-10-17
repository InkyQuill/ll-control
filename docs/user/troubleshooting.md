# Troubleshooting and backup

| Problem | Try this |
|---|---|
| Application says disconnected | Check `systemctl --user status lconnectd`. Run the GUI and daemon as the same user. |
| Device missing or permission denied | Check that it is LCDV2 and connected. After a new install, restart the computer when convenient so device permissions are applied. Do not run a second daemon. |
| Fans do not respond | Confirm they are connected to the LCDV2 controller, not the motherboard. |
| Sensor shows a dash | The reading is missing or stale. Check the selected source; do not treat it as zero. |
| Apply/Save result is uncertain | Refresh or read the operation result before retrying. Avoid repeating an import blindly. |
| Referenced image/font cannot be removed | Remove or change references in profiles/compositions first. |
| Capture cancellation failed | Use the offered cleanup retry; do not start another capture until cleanup completes. |

For a bug report, include the application version, device/firmware, desktop
session, steps and error message. Recent service logs:
`journalctl --user -u lconnectd -n 100 --no-pager`.
Review logs for personal information before sharing.

## Back up settings

Stop the daemon, then copy the entire `~/.config/lconnectd` directory, including
its managed images, fonts and clips. If you set `XDG_CONFIG_HOME`, use the
`lconnectd` folder there instead. Restart the daemon afterward.

To restore, stop the daemon and replace that directory with your backup.
Starting it may reapply saved settings. Do not delete settings as a general fix
for a single error. Native profile backups can also be created in the application;
they are not Windows-compatible exports.
