# LL-Control

**ll-control** (UI: **LL-Control**) is an independent Rust application for Lian Li
cooling, lighting and LCD content on Linux, based on the original .NET L-Connect 3
behavior. The user's physical target is **Galahad II LCDV2 (`0416:7395`), square
480×480 LCD**. The broader goal includes applicable device parity and all functions
of the separate third-party [ll-connect3](https://github.com/joeytroy/ll-connect3),
including UNI FAN SL-INF.

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
available. Other parity devices and third-party functions are unverified; use of
unverified hardware is at the user's own risk. Scope does not mean a function is
implemented or usable, and software tests alone do not establish hardware support.
See the
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
