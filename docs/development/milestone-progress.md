# Milestone progress

## R0 — release baseline and repository organization

Completed the source audit, immutable build-input baseline and release feature
contract. Initial target: LCDV2 / Arch x86_64 / Plasma Wayland, with EN/RU and
basic direct canvas editing required. Final release qualification remains R3.

The R0 commit also records the substantial pre-existing implementation in the
working tree. It does not claim that work was newly developed in this milestone.
Its source baseline and earlier verification were checked before documentation
migration; no hardware run occurred here.

Developer contracts, plans and reference material are under `docs/development/`.
Users have short install/use/troubleshooting guides under `docs/user/`. Raw run
reports, screenshots, captures and checkpoint logs are Git-ignored and preserved
locally. Reproducible tests, fixtures, tools and license provenance stay tracked.
Historical Git objects were not rewritten.

Validation: local document links checked; tracked evidence inventory checked;
production edits in this organization step are reference-path comments only.
The existing candidate's saved checks are historical evidence, not a new suite run.
R1 implementation and interaction acceptance are next and receive their own commit.
