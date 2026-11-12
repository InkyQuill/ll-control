# Releasing ll-control

The product has one version and one GitHub release, with tags such as `v0.1.0`.
The Arch compatibility package remains named `lconnect-linux`. The package builder
reads `[workspace.package].version` from `Cargo.toml`; it generates the versioned
payload archive, manifest and PKGBUILD from that value.

## Propose and publish

1. Merge conventional commits into `master`. Use squash titles such as
   `fix: repair font selection` and `feat: add an editor capability`.
2. Release Please opens or refreshes its release PR. Review its changelog,
   version changes and successful **Linux checks** run on
   the latest proposal commit. Do not merge while CI is pending.
3. Merge the release PR. Release Please creates the `vX.Y.Z` tag and GitHub
   release, then calls `release-arch.yml` with that tag to run full quality
   checks, build, test and attach the Arch package. It does not depend on a token-generated tag/release event.

The initially empty `.release-please-manifest.json` means no release has been
published. `initial-version: 0.1.0` makes the first release `0.1.0` and preserves
the available conventional-commit history rather than starting after this rollout.
There is no permanent `release-as` override. After the first release, fixes bump
the patch version and features bump the minor version. Breaking changes also
bump the minor version while the product is below `1.0.0`. An intentional 1.0
release can use Release Please's `Release-As: 1.0.0` commit footer.

## Versions and lockfile

The root `simple` release strategy updates `version.txt`. Explicit TOML extra-file
updaters update `$.workspace.package.version` in `Cargo.toml` and
`$.package[?(!@.source)].version` in `Cargo.lock`. The latter selects every local
workspace package and leaves third-party versions, checksums and other bytes
unchanged. JSON extra-file updaters synchronize `desktop/package.json` and the
Tauri configuration. All workspace crates inherit the root version.
`pnpm-lock.yaml` does not contain the root package version and needs no
release-only rewrite.

The upstream Rust strategy requires `[package]`, and its workspace plugin expects
literal member versions. This repository has a virtual workspace and inherited
versions, so a single product release with explicit native updaters is used.
There is no additional lock mutation or commit after Release Please's proposal.

## Ordinary GitHub token and CI

No personal token or App secret is required. Enable **Allow GitHub Actions to
create and approve pull requests** in repository Actions settings; organization
policy must permit it. The workflow explicitly grants contents, pull-request
and Actions write permissions only to its proposal job.

GitHub-token PR events can require human approval, and token-created pushes do
not generally start workflows. The proposal workflow explicitly dispatches
`linux.yml` and the lightweight release configuration checks on the final PR head, without running proposal code in the write-token job. Before dispatching it
checks for an already queued/running or successful Linux run on that exact SHA,
so a previously approved/validated proposal is reused. Use that dispatched run
instead of separately approving another copy of the same checks. Publication
uses a direct reusable-workflow call; there is no second release/tag trigger.

Regular Linux CI runs browser checks and verifies that the production-feature
Arch binaries build; it does not run the full workspace test, Clippy and docs
suite or produce distribution artifacts. The release workflow runs that full
suite through `quality.yml` against the release tag before packaging. Run
**Full quality checks** manually with an optional source ref when those checks
are needed independently of a release.

## Recovery and verification

Rerun **Release Please** on `master` to refresh a proposal or retry release
creation. It tracks release lifecycle labels and existing tags rather than
creating another release for the same merged PR. If a release exists but package
publication failed, manually run **release-arch.yml** with its existing tag.
The package workflow checks existing assets before upload and rejects differing
bytes rather than overwriting them.

Run `python3 tools/tests/check-release-versions.py` to check current version agreement.
Release configuration CI
tests the actual `release-please@17.1.3` implementation bundled by pinned action
v4.4.0: first `0.1.0`, patch `0.1.1`, feature `0.2.0`, breaking `0.3.0`, together
with markerless root TOML and byte-preserving lock fixtures. Update both pins and rerun these checks when updating
the release action.

References: [Release Please action](https://github.com/googleapis/release-please-action),
[custom updaters](https://github.com/googleapis/release-please/blob/main/docs/customizing.md),
and [GitHub token workflow triggers](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow).
