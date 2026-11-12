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
