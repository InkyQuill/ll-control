# L-Connect Linux

Control Lian Li cooling, lighting and LCD content from Linux.
Current device target: **Galahad II LCDV2 (`0416:7395`), square 480×480 LCD**.

**Software preview:** core workflows are implemented, but current hardware and
real desktop acceptance are incomplete. The first release targets **KDE Plasma
Wayland on Arch Linux x86_64**. English and Russian are available.

- [Install and start](docs/user/install.md)
- [Use the application](docs/user/use.md)
- [Troubleshooting and backup](docs/user/troubleshooting.md)
- [Developer guide](docs/development/README.md)
- [Roadmap and milestones](roadmap.md)

Screenshot/recording, Windows import, procedural effects and Mini/FPS integrations
are experimental. Firmware flashing and Windows-compatible export are not yet
available. Other Lian Li devices are not qualified. See the
[release support table](docs/development/parity/release-contract.md).

## Development

The application uses Rust, a local daemon and a Svelte/Tauri desktop interface.
[Build and verification instructions](docs/development/README.md) explain how to
work without accessing physical hardware. Windows L-Connect3 2.1.28.0 is the
capability reference; proprietary assets are not included.

Code, tests, test fixtures, developer contracts and milestone records belong in
Git. Raw logs, screenshots, captures and generated acceptance reports stay local
and ignored. See [documentation and evidence policy](docs/development/README.md#repository-policy).

## License

See [LICENSE](LICENSE). Bundled fonts and assets retain their own notices.
