# Implementation status

Last updated: 2026-08-22T19:20:52-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Current implementation SHA: `c676cfd732d02d6202bff8cea47fcf74b5bfd8e7`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-03 artifact catalog and immutable-table contract activation

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `C-01`, `C-02`, `FND-01`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`, `WS-20`, `WS-21`
- Active: `C-03`, `DATA-01`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`
- Planned next: `C-04`–`C-06`

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all five archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. C-02 otherwise closes with 440/440 all-feature workspace tests, both warnings-denied Clippy rows, nightly fuzz, docs, and compatibility gates. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Dirty files and reasons

- Production, tests, manifests, and locks are clean at `c676cfd732d02d6202bff8cea47fcf74b5bfd8e7`. Current dirty files are C-02 closure evidence only: handoff, decision/package finding, validation ledger, ownership/status, and task-contract status.

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
- `DEC-0016`: only patient/specimen objects may be biological units; nearest resolution and bounded enclosing lineage coexist so downstream designs select a level explicitly without pseudoreplication.
- `DEC-0017`: C-02 uses named directed affine maps and explicit parallel-section placement; missing calibration, inverse/path selection, and uncertainty propagation are never inferred.
- `DEC-0018`: keep unpublished workspace registry resolution explicit; supplemental local patch verification cannot turn the exact package gate green, and no version/publish action is inferred.

## Unresolved questions

- Stable cross-artifact `CellId` alignment for remote CellViT JSON/`.pt`/NPY rows.
- Complete extraction-layer/pooling/normalization metadata and a trusted `.pt`-to-Arrow/Parquet converter.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Audit current B-04 `ArtifactRef`/digest/project ownership, C-01/C-02 durable-reference needs, existing Arrow/Parquet dependencies, filesystem transaction policy, and authorized remote provenance without reading executable `.pt` payloads.
2. Freeze C-03 schemas, digest/version semantics, local/object-store boundary, partial-write recovery, symlink/path controls, and migration rejection in a task contract before production edits.
3. Add failing digest/schema/partial-write/stale-cache/path-boundary tests, then implement the smallest immutable artifact catalog and table-manifest slice.

Next exact verification command:

```bash
rg -n "ArtifactRef|ContentDigest|OutputWriter|Parquet|object.store|symlink|canonical" crates src tests docs/implementation
```
