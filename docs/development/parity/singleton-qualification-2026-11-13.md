# Daemon singleton host qualification — 2026-11-13

Issue: [#1](https://github.com/InkyQuill/lconnect-linux/issues/1).
Base revision: `338886077f6c63d98cc13daa053824a89287c2eb`; this change adds
`crates/lconnect-daemon/tests/startup_ownership.rs` without changing production
startup or socket ownership code.

The production executable now has a regression for its losing startup path.
The test holds `DaemonOwnership` in a socket-only owner fixture, connects an
existing client, and launches the actual built `lconnectd` with isolated XDG
runtime, configuration, data and state directories. The child must exit within
ten seconds with the ownership refusal and the explicit before-persistence/HID/
replay diagnostic. No data or state directory may be created, and the
post-acquisition startup log must be absent. The original socket inode remains,
the existing client still exchanges data, a new client connects, and dropping
the owner removes its own socket and releases the lock for acquisition again.

This complements the IPC suite's two-process owner/loser fixture, stale socket
replacement, changed-inode cleanup protection, live and silent accepted peer
protection, and bounded nonblocking socket probes. The fixture's marker checks
owner work happens only for the winner. The production test checks the actual
binary takes its early refusal path; it does not instrument physical HID calls.
No winning production daemon, sensor collector, or hardware replay was started.

Reproduce from the repository root:

```sh
cargo test --locked -p lconnect-ipc
cargo test --locked -p lconnect-daemon --test startup_ownership
cargo clippy --locked -p lconnect-daemon --test startup_ownership -- -D warnings
cargo fmt --all --check
```

Observed on Linux with local Unix sockets: IPC suite **111 passed**, no failures
or ignored tests; production startup regression **1 passed**, no failures or
ignored tests; targeted Clippy and workspace formatting passed. The Cargo target
directory was a shared local build cache; the executable under test was rebuilt
from this worktree by Cargo.

This qualifies host-side singleton and cleanup behavior on this source plus the
added test. It does not qualify an installed release package, another runtime
directory, the separate .NET implementation, real desktop lifecycle, or physical
device ownership. Repeat these checks on the combined release revision; package
qualification remains under issue #5.
