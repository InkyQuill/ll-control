# Developer guide

Start with [PRODUCT](../../PRODUCT.md), [roadmap](../../roadmap.md) and the
[R0–R10 milestone plan](parity/release-milestones.md). Current product behavior is
in [implementation status](parity/implementation-status.md); detailed contracts
are grouped under [parity](parity/). [Protocol research](protocol/galahad2-lcdv2.md)
and the [versioned reference](2026-09-08-reference-2.1.28-and-ll-connect3.md)
explain the Windows baseline.

## Build and test

- Rust: `cargo test --workspace --locked`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Frontend: `pnpm --dir desktop install --frozen-lockfile`, then `check`, `build`,
  `check:locales` and `check:contracts` using that same directory.
- Browser interactions: `pnpm --dir desktop exec playwright test`.
- Native app: [desktop development](../../desktop/README.md),
  [isolated native testing](parity/native-desktop-smoke.md) and
  [keyboard acceptance](parity/native-keyboard-acceptance.md).
- Delivery: [package builder and consumer checks](parity/package-delivery.md).

A production daemon can write hardware at startup through saved-state replay.
Use the private MockBackend/Xvfb harness for native verification. Hardware tests
have a separate [acceptance protocol](parity/lcdv2-hardware-acceptance.md).

[CLI/reference notes](cli-reference.md) and the older
[extended technical quickstart](quickstart.md) preserve implementation history;
consult the current contracts when a dated checkpoint conflicts with current code.
For ordinary installation/use, send users to the [short user guides](../user/install.md).

## Repository policy

Track code, reproducible tests and owned fixtures, build/verification tools,
product decisions, contracts and milestone progress. Commit each completed
milestone separately with its result and remaining boundaries in the commit body.
Preserve prior work; do not claim unrun verification.

Do **not** track generated run evidence: logs, screenshots, packet captures,
performance runs, source-hash inventories or acceptance reports. Existing evidence
is retained locally under ignored `parity/evidence/`, protocol `captures/`,
checkpoint ledgers and `.impeccable/review/`. Such links in developer history are
local references and intentionally unavailable in a clean clone. Historical Git
commits are not rewritten. Tests/fixtures and asset-license provenance are source
inputs, not disposable run evidence.

Keep [user guides](../user/) short, task-oriented and free of implementation
history. Put architecture, debugging, acceptance criteria and milestone details
here. Root README is an entry point, not a combined manual and work log.
