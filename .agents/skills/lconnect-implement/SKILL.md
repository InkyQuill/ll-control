---
name: lconnect-implement
description: Implement or fix L-Connect Linux features across protocol, HID, daemon, IPC, CLI, and image preparation. Use for code changes or execution of an approved milestone, not research-only audits.
---

# L-Connect implementation

Read `roadmap.md` from the repository root and the plan relevant to the requested change. The roadmap supersedes the old phase ordering. Inspect current code and dirty files before choosing edit boundaries; plans and historical test counts are not proof of today's implementation.

## Working prompt

Complete the requested change through appropriate verification. Resolve routine choices from existing code; ask only when a missing decision affects scope or correctness. Preserve prior authorization and unrelated edits. User instructions override this skill's workflow preferences. If a skill blocks progress, identify its file and exact rule. Keep reports concise and evidence-based. Delegate only when authorized and useful; give each agent a bounded responsibility and retain integration ownership. After relevant checks pass, repeat or expand them only for a concrete unresolved concern.

## Project decisions

- Trace the affected path across `crates/protocol`, `crates/lconnect-hid`, `crates/lconnect-daemon`, `crates/lconnect-ipc`, `crates/lconnect-cli`, and `crates/lconnect-image`. Read the affected crate manifests for package names.
- Keep wire encoding in protocol, device access in HID, and persistent device work in the daemon. Inspect actor/keeper ownership before changing cancellation, replay, or streaming; avoid competing device writers.
- Match the user-visible LCDV2 scenario in the current roadmap. Do not turn a small milestone into support for other device families or an unsolicited GUI scaffold.
- For a protocol change, locate the relevant versioned reference and fixture. Firmware-dependent framing and successful USB writes require separate consideration from visible playback.
- For udev changes, inspect `xtask/src/main.rs`; `cargo run -p xtask -- gen-udev` generates the packaged rule. Updating a generated rule alone leaves its source inconsistent.

## Verification and delivery

Use a focused regression test when it captures a real failure: malformed packet rejection, preserved neighboring settings, socket interoperability, cancellation, or persistence behavior. Run affected crate tests and relevant formatting/lint checks. Broaden to workspace checks for shared interfaces or cross-crate changes. A docs-only edit needs document validation, not hardware execution.

Do not start a daemon merely to test compilation: startup may replay saved settings. For live verification, consult `docs/parity/lcdv2-hardware-acceptance.md` and the `lconnect-hardware` skill. If hardware is unavailable, finish host-side work and report the unverified acceptance scenario precisely.

Report changed behavior, checks actually run, and remaining limitations. Never describe codec/mock success as hardware acceptance. Commit, publish, or install only within the user's requested scope.

Prompting basis: [OpenAI GPT-6 Astra recommendations](https://developers.openai.com/api/docs/guides/latest-model#prompting-best-practices), reviewed 2026-09-08. The project workflow above is a local adaptation; this skill does not select a model.
