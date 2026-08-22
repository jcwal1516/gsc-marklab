# Implementation status

Last updated: 2026-08-22T16:46:19-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Current SHA: `fc986c0c4e06216cc85d55d2315482a0107bf5b7`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 1 / WS-B / B-01 workspace architecture decision

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `SLIDE-INV`, `WS-A`
- Active: `B-01`, `PLAT-01`, `WF-01`
- Planned next: `B-02`, `B-03`, `B-04`

## Command state

- Known failing commands/tests: none current. Historical WS-A harness failures are recorded in the validation ledger: one zsh wrapper error and one expected dirty-tree package refusal; both corrected/rerun successfully.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Dirty files and reasons

- `docs/implementation/STATUS.md`, `REQUIREMENTS.md`, `VALIDATION_LEDGER.md`, and A-01/A-02/A-03 handoffs: WS-A closure evidence pending one documentation-only closure commit.

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

1. Commit the WS-A closure handoffs/evidence and confirm clean status.
2. Freeze B-01 writable files/symbols and its workspace dependency decision.
3. Add a behavior-first workspace characterization test, then implement the smallest passing workspace boundary.

Next exact verification command:

```bash
git diff --check && git status --short
```
