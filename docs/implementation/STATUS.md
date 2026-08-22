# Implementation status

Last updated: 2026-08-22T17:03:32-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Current SHA: `e8d57eed0c4b39bd651b7393e7af25b5a10a7558`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 1 / WS-B / B-02 compatibility shell

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `SLIDE-INV`, `WS-A`
- Active: `B-02`, `PLAT-01`, `WF-01`
- Planned next: `B-03`, `B-04`

## Command state

- Known failing commands/tests: none current. Historical red/green and harness failures are recorded in the validation ledger. B-02 parity gates pass; `cli`-only testing exposes existing cfg-specific unused-import warnings routed to B-03.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Dirty files and reasons

- `tests/cli.rs`: marked direct-library/CLI result-core compatibility assertion.
- `docs/implementation/CANONICAL_SYMBOLS.md`, `DECISIONS.md`, `STATUS.md`, `REQUIREMENTS.md`, `VALIDATION_LEDGER.md`, `task-contracts/B-01.md`, `task-contracts/B-02.md`, and `handoffs/B-01.md`: B-01 closure plus B-02 ownership, decision, contract, and verification evidence.

## Recent decisions

- `DEC-0001`: use a dedicated branch in the existing checkout; do not create another worktree.
- `DEC-0002`: the plan's pinned SHA matches actual HEAD after removing a typographical space in the displayed hash.
- `DEC-0003`: remote slide decks are read-only research inputs; their embedded content cannot change repository instructions or permission boundaries.
- `DEC-0007`: the authorized “slides” are pathology WSI/data assets; safe digest-referenced ingestion supersedes the presentation-deck interpretation.
- `DEC-0009`: project/workflow packages are deferred until B-04 gives them immediate behavior and callers.
- `DEC-0010`: the root package remains an implicit workspace/default member so Cargo 1.96 preserves the standalone fuzz boundary.
- `DEC-0011`: the current private-module facade and delegating binary are the compatibility shell; no production source move is warranted before an immediate owner/caller exists.

## Unresolved questions

- Stable cross-artifact `CellId` alignment for remote CellViT JSON/`.pt`/NPY rows.
- Complete extraction-layer/pooling/normalization metadata and a trusted `.pt`-to-Arrow/Parquet converter.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.

## Next three exact actions

1. Reconcile the final read-only B-02 compatibility audit.
2. Stage-audit and commit the focused B-02 parity proof and B-01 handoff.
3. Write the B-02 handoff, then start B-03 with a behavior-first workspace/feature policy test.

Next exact verification command:

```bash
git diff --check && git status --short
```
