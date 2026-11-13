# Rust advisory and dependency updates

The Rust advisory workflow audits the committed workspace `Cargo.lock` on pushes, pull
requests, manual runs, and weekly. It installs a pinned `cargo-audit` release
with `--locked` and checks the lockfile against the current RustSec advisory
database. The audit fails on reported vulnerabilities; warnings such as
unmaintained crates also need review. An unavailable advisory database or failed
tool installation is a failed check, not a clean audit. Run the same check
locally with `cargo install cargo-audit --version 0.22.1 --locked` followed by
`cargo audit --file Cargo.lock`.

Review dependency and audit-tool updates in a pull request. Inspect the lockfile
diff and upstream release notes, identify affected direct and transitive
dependencies, and run the audit and applicable build and test checks. Reviewers
should assess reported advisories, including reachability and available fixes;
prefer upgrading or removing affected dependencies. If a fix is unavailable,
document the advisory ID, affected versions, impact, mitigation, owner, and
revisit date in the pull request. Any audit exception must be narrowly scoped,
justified and reviewed in version control, and removed when no longer needed;
do not suppress findings merely to make CI green. Re-run the audit after
updating the tool or dependencies. Scheduled failures should be triaged even
when no dependency changed, since advisories change independently.

This audit covers Rust packages represented in `Cargo.lock`. Native libraries,
system packages and sysroot qualification require separate review and testing;
a clean Rust audit does not qualify those dependencies.

## Lockfile triage — 2026-11-13

The locked audit after these changes reports **11 allowed warnings**, reduced
from 14 without an `audit.toml` or any advisory suppression. `rustix` was updated
to 1.1.6 for RUSTSEC-2026-0486, Tokio to 1.53.3 for RUSTSEC-2026-0386, and
`async-executor` to 1.14.1 to remove the yanked 1.14.0 package. These cover both
shipped code and build-only dependency paths. Revisit the remaining items by
**2026-12-13**, or earlier when an upstream compatible upgrade becomes available.

| Advisory / affected packages | Owner and applicability | Disposition and next action |
|---|---|---|
| RUSTSEC-2024-0429 — `glib` 0.18.5, unsound `VariantStrIter` | Transitive through the shipped Tauri/GTK/WebKit and GStreamer stack. LL-Control has no direct `VariantStrIter` calls; reachability inside native bindings is not assumed away. | **Deferred, highest priority.** Patched GLib is >=0.20.0, which requires a coordinated major GUI-stack migration and real-session tests. Do not force an incompatible point upgrade or suppress the warning. |
| RUSTSEC-2024-0436 — `paste` 1.0.15, unmaintained | Build-time macro dependency of `option-operations`/GStreamer 0.21.3; generated code reaches the GUI, while the macro itself runs during builds. | Track a GStreamer upgrade containing a maintained replacement. No direct compatible replacement is currently declared. |
| RUSTSEC-2024-0370 — `proc-macro-error` 1.0.4, unmaintained | Build-time macro dependency of GTK/GLib 0.18; the macro runs during builds and generated code reaches the GUI. | Migrate together with the GTK/GLib major upgrade. No independent upgrade is forced. |
| RUSTSEC-2026-0206 — `rustybuzz` 0.20.1, unmaintained | Direct `lconnect-image` text-shaping dependency used by shipped renderers and their fixtures. | Migrate the font stack to `harfrust` and rerun raster/locale fixtures. Treat shaping differences as behavior changes, not a mechanical rename. |
| RUSTSEC-2026-0192 — `ttf-parser` 0.25.1, unmaintained | Transitive through `rustybuzz` and `ab_glyph`/`owned_ttf_parser` in `lconnect-image`, used by shipped renderers and fixtures. | Coordinate with the shaping migration and evaluate `skrifa` plus an `ab_glyph` replacement. Migrating only `rustybuzz` would leave this warning. |
| RUSTSEC-2026-0371 — `zbus_names` 3.0.0 and 4.3.4, unmaintained | Transitive through shipped daemon/desktop `zbus` 4.4 and the single-instance plugin's `zbus` 5.19 paths. | Migrate to `zbus` 6's `names` module together with the daemon, desktop, and plugin APIs. Standalone crates have no patched release. |
| RUSTSEC-2026-0370 — `zvariant` 4.2.0 and 5.15.0, unmaintained | Transitive through the same shipped D-Bus paths. | Migrate to `zbus` 6's `wire` module; evaluate `zgvariant` only if GVariant is actually required. Standalone crates have no patched release. |
| RUSTSEC-2026-0372 — `zvariant_derive` 4.2.0 and 5.15.0, unmaintained | Build-time derive dependency of the same D-Bus paths; generated code reaches shipped binaries. | Covered by the coordinated `zbus` 6 migration. Do not leave the derive crates behind when updating runtime crates. |

The remaining warnings are accepted only as documented upstream-maintenance
risks, not as proof of safety. No audit finding is suppressed, and no hardware or
real-session acceptance is implied by dependency tests.
