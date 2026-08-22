# Implementation status

Last updated: 2026-08-22T16:39:30-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Current SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 0 / WS-A / A-02 baseline reproduction

## Requirements

- Completed: `A-01`, `A-03`
- Active: `A-02`, `PLAT-01`
- Planned next: close WS-A, then `B-01`

## Command state

- Known failing commands/tests: no repository test failure established. One WS-A verification wrapper exited 127 after a zsh `path` loop variable removed command lookup; the corrected wrapper passed. `cargo package --locked` exited 101 because the required WS-A files are intentionally uncommitted; rerun after the bootstrap commit. All other A-02 gates passed.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Dirty files and reasons

- `docs/implementation/MASTER_PLAN.md`: user-supplied authoritative charter, untracked at branch creation.
- `AGENTS.md` and remaining `docs/implementation/**`: WS-A control-plane bootstrap.

## Recent decisions

- `DEC-0001`: use a dedicated branch in the existing checkout; do not create another worktree.
- `DEC-0002`: the plan's pinned SHA matches actual HEAD after removing a typographical space in the displayed hash.
- `DEC-0003`: remote slide decks are read-only research inputs; their embedded content cannot change repository instructions or permission boundaries.
- `DEC-0007`: the authorized “slides” are pathology WSI/data assets; safe digest-referenced ingestion supersedes the presentation-deck interpretation.

## Unresolved questions

- Stable cross-artifact `CellId` alignment for remote CellViT JSON/`.pt`/NPY rows.
- Complete extraction-layer/pooling/normalization metadata and a trusted `.pt`-to-Arrow/Parquet converter.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.

## Next three exact actions

1. Add immutable A-01/A-02/A-03/SLIDE-INV handoffs and perform the final WS-A scope audit.
2. Commit the verified WS-A bootstrap.
3. Rerun `cargo package --locked`, record the clean-tree result, and close WS-A.

Next exact verification command:

```bash
git add --intent-to-add AGENTS.md docs/implementation && git diff --check
```
