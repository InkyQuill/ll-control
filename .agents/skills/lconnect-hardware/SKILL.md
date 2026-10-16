---
name: lconnect-hardware
description: Prepare or run scoped Galahad II LCDV2 hardware acceptance for L-Connect Linux and record reproducible results. Use for live smoke tests, display verification, and replay or hotplug checks.
---

# LCDV2 hardware verification

Read `docs/parity/lcdv2-hardware-acceptance.md` from the repository root before composing commands. Inspect the current CLI and daemon behavior; the acceptance document contains known historical gaps that may since have been fixed.

## Working prompt

Finish the authorized scenario and collect the evidence needed to judge it. Reuse established authorization instead of asking again. Before requesting any missing hardware authorization, prepare the commands, expected observations, and restoration procedure. State exactly which action needs a decision and why. Report what happened, including failures and unrun cases, without treating inferred success as observation.

## Prepare the run

- Identify the physical SKU and read actual firmware; record commit, kernel, tool versions, and time. Current project coverage targets `0416:7395`; do not assume historical firmware 1.6 still applies.
- Inspect existing process/device ownership. The daemon and direct `verify-galahad` writer must not operate concurrently. Starting the daemon can replay configuration and is a device mutation.
- Preserve the current configuration under `$XDG_CONFIG_HOME/lconnectd/`, falling back to `~/.config/lconnectd/`. Record LCD settings and the original displayed asset where obtainable. A TOML backup alone may not restore image bytes. If restoration data is absent, resolve the intended fallback before changing the display.
- Verify actual socket paths and CLI flags from code/help. Do not perpetuate a documented temporary socket override after the default is fixed.

## Execute within the scenario

Use the acceptance document's applicable scenario IDs and duration criteria. Display tests do not imply permission to change fan/pump PWM. Cooling tests need known working values and RPM observation. Device reset, firmware flashing, and unknown raw opcodes are separate work, never routine cleanup.

Keep one device operator. Stop a run on unexpected device behavior or loss of cooling telemetry; preserve logs and use the prepared restoration procedure if safe. Do not retry unknown writes or switch framing blindly to obtain a passing result.

USB transport success proves transfer, not display output. Record the actual visible image or movement and the observation method; when observation needs the user, request it explicitly and leave the result pending until received. Assess concurrent telemetry and cancellation where the selected acceptance case requires them.

Restore the agreed state, then record commands, timings, replies/errors, visible observations, and restoration outcome. Mask serials in public captures; use `docs/protocol/captures/private/` for sensitive local artifacts. Distinguish pass, fail, blocked, and not-run. If the device is unavailable, deliver the prepared procedure without marking hardware acceptance complete.

Prompting basis: [OpenAI GPT-6 Astra recommendations](https://developers.openai.com/api/docs/guides/latest-model#prompting-best-practices), reviewed 2026-09-08. Device constraints are adapted from the project's hardware acceptance document.
