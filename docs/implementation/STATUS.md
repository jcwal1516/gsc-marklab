# Implementation status

Last updated: 2026-08-22T17:13:00-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Current SHA: `f1bcc94d4a5f96825fae32676630304f31d332ea`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 1 / WS-B / B-03 workspace policy

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `SLIDE-INV`, `WS-A`
- Active: `B-03`, `PLAT-01`, `WF-01`
- Planned next: `B-04`

## Command state

- Known failing commands/tests: none current. Historical red/green and harness failures are recorded in the validation ledger. B-03's compile matrix passes; narrow no-default/CSV/Parquet/WSI all-target checks retain explicitly recorded warnings and are not claimed warning-clean.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Dirty files and reasons

- `.github/workflows/**`, `AGENTS.md`: workspace/package-explicit gates and eight-row feature matrix.
- `tests/workflow_contract.rs`, `tests/workspace_contract.rs`: workflow, dependency-layer, and recursive CLI-gating policy.
- `src/cli/batch.rs`: cfg-correct imports for CLI without parallel execution.
- `docs/implementation/WORKSPACE_POLICY.md`, `REPOSITORY_MAP.md`, `CANONICAL_SYMBOLS.md`, `DECISIONS.md`, `STATUS.md`, `REQUIREMENTS.md`, `VALIDATION_LEDGER.md`, `task-contracts/B-02.md`, `task-contracts/B-03.md`, and `handoffs/B-02.md`: B-02 closure and B-03 policy/evidence.

## Recent decisions

- `DEC-0001`: use a dedicated branch in the existing checkout; do not create another worktree.
- `DEC-0002`: the plan's pinned SHA matches actual HEAD after removing a typographical space in the displayed hash.
- `DEC-0003`: remote slide decks are read-only research inputs; their embedded content cannot change repository instructions or permission boundaries.
- `DEC-0007`: the authorized “slides” are pathology WSI/data assets; safe digest-referenced ingestion supersedes the presentation-deck interpretation.
- `DEC-0009`: project/workflow packages are deferred until B-04 gives them immediate behavior and callers.
- `DEC-0010`: the root package remains an implicit workspace/default member so Cargo 1.96 preserves the standalone fuzz boundary.
- `DEC-0011`: the current private-module facade and delegating binary are the compatibility shell; no production source move is warranted before an immediate owner/caller exists.
- `DEC-0012`: dependencies descend root facade → generic workflow → project; B-04's concrete engine node adapter stays in root.

## Unresolved questions

- Stable cross-artifact `CellId` alignment for remote CellViT JSON/`.pt`/NPY rows.
- Complete extraction-layer/pooling/normalization metadata and a trusted `.pt`-to-Arrow/Parquet converter.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.

## Next three exact actions

1. Reconcile the final read-only B-03 policy audit and review the complete diff.
2. Stage-audit and commit the focused B-03 policy change plus B-02 handoff.
3. Rerun clean `cargo package --locked --workspace`, write the B-03 handoff, and begin B-04 red tests.

Next exact verification command:

```bash
git diff --check && git status --short
```
