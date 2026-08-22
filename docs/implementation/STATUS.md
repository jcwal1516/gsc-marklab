# Implementation status

Last updated: 2026-08-22T18:13:37-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Current SHA: `3ea2e9849ae024759d871d1e6ce6a566d2c40d92`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-01 typed identities and hierarchy activation

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`
- Active: `C-01`, `DATA-01`, `FND-01`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`, `WS-20`
- Planned next: `C-02`–`C-06`

## Command state

- Known failing commands/tests: none current. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix passes; narrow no-default/CSV/Parquet/WSI all-target checks retain explicitly recorded warnings and are not claimed warning-clean.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Dirty files and reasons

- `docs/implementation/CANONICAL_SYMBOLS.md`, `DECISIONS.md`, and `task-contracts/C-01.md`: C-01 ownership, audited frozen semantics, and the TMA-safe containment/source decision. Production and test trees are clean at `3ea2e9849ae024759d871d1e6ce6a566d2c40d92`.

## Recent decisions

- `DEC-0001`: use a dedicated branch in the existing checkout; do not create another worktree.
- `DEC-0002`: the plan's pinned SHA matches actual HEAD after removing a typographical space in the displayed hash.
- `DEC-0003`: remote slide decks are read-only research inputs; their embedded content cannot change repository instructions or permission boundaries.
- `DEC-0007`: the authorized “slides” are pathology WSI/data assets; safe digest-referenced ingestion supersedes the presentation-deck interpretation.
- `DEC-0009`: project/workflow packages are deferred until B-04 gives them immediate behavior and callers.
- `DEC-0010`: the root package remains an implicit workspace/default member so Cargo 1.96 preserves the standalone fuzz boundary.
- `DEC-0011`: the current private-module facade and delegating binary are the compatibility shell; no production source move is warranted before an immediate owner/caller exists.
- `DEC-0012`: dependencies descend root facade → generic workflow → project; B-04's concrete engine node adapter stays in root.
- `DEC-0013`: project inputs remain reference-only; only bounded canonical result-0.3 bytes and successful-run records are retained in B-04.
- `DEC-0014`: use the already-locked reviewed `sha2` 0.10.9 and `thiserror` 2 dependencies; no registry lock delta is accepted.
- `DEC-0015`: C-01 separates compact containment parents from explicit biological sources so donor cores on multi-donor TMA slides remain attributable without filename inference or false technical-replicate labels.

## Unresolved questions

- Stable cross-artifact `CellId` alignment for remote CellViT JSON/`.pt`/NPY rows.
- Complete extraction-layer/pooling/normalization metadata and a trusted `.pt`-to-Arrow/Parquet converter.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.

## Next three exact actions

1. Add the C-01 workspace/package and root integration contracts, then confirm the missing members/API fail for the expected reasons.
2. Implement core IDs, indexed hierarchy validation/design summaries, and atomic project hierarchy installation in dependency order.
3. Add and run the correctness-checked hierarchy benchmark plus focused workspace/documentation/lock gates.

Next exact verification command:

```bash
cargo +1.96.0 test --locked --test data_hierarchy
```
