---
name: lconnect-parity
description: Research L-Connect Linux LCDV2 parity against versioned Windows behavior and produce evidence, gap matrices, or milestone plans. Use for protocol investigation and audit requests without implicitly starting implementation.
---

# LCDV2 parity research

## Working prompt

Carry the requested investigation through to a usable finding or plan. Make routine research choices independently. Respect a research-only boundary even when a defect is easy to fix. Explain conclusions in plain language with exact evidence; separate observed behavior from inference. Apply the user's instructions ahead of this skill's preferences, and identify the specific rule if guidance causes a pause.

## Locate the baseline

Paths below are relative to the repository root. Read `roadmap.md` first, then only the relevant sections of:

- `docs/development/2026-09-08-reference-2.1.28-and-ll-connect3.md` for reference locations, assembly provenance, and decompilation limits.
- `docs/development/parity/lcdv2-2.1.28-matrix.md` for scenario coverage.
- `docs/development/parity/lcdv2-baseline-gaps.md` for recorded implementation gaps.
- `docs/development/protocol/lcdv2-screen-pipeline-2.1.28.md` for display-path investigation.

The recorded baseline is Windows 2.1.28.0 for Galahad II LCDV2 `0416:7395`. Verify the applicable reference version before using it; do not silently mix older decompilation with this baseline. Treat the root roadmap as the current ordering, and old phase plans as historical context unless the user selects them.

## Investigate a scenario

Follow the user action through Windows UI/service/Core, protocol framing, and the matching Linux path. Record source paths, symbols, version or hash provenance where available, and the resulting behavioral requirement. A command enum or matching method name does not establish identical firmware behavior. Keep proprietary reference exports outside the Linux repository.

Classify evidence as source-derived, codec-tested, mock/integration-tested, historical hardware observation, or current hardware observation. Record unknown applicability as unresolved rather than omitting it. Do not promote existing captures to a fresh hardware result.

For each actionable gap, give the scenario, current behavior, evidence, affected files, dependencies, and observable acceptance criteria. Use a comparison table when it helps. Update a requested matrix or plan narrowly; a completed proposal does not itself authorize implementation. Do not run device commands as part of a source audit.

When parallel research is authorized, split independent scenarios or reference layers and reconcile their evidence into one result. Do not distribute concurrent live device access.

Prompting basis: [OpenAI GPT-6 Astra recommendations](https://developers.openai.com/api/docs/guides/latest-model#prompting-best-practices), reviewed 2026-09-08. Project evidence rules come from the local roadmap and parity documents.
