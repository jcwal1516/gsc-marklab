# Implementation status

Last updated: 2026-08-23T04:12:10-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Committed baseline before the current checkpoint: `49ed11397eadf1746083e69422d9892a81845adb`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-04 partial streaming scan/QC and verified compact-artifact checkpoint

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `C-01`, `C-02`, `C-03`, `FND-01`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`, `WS-20`, `WS-21`
- Active: `C-04`, `DATA-01`, `EMB-CORE`, `FND-05`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`, `WS-24`
- Planned next: `C-05`–`C-06`

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all five archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. The Windows cross-target C-03 check also exits 101 before project compilation because `x86_64-pc-windows-msvc` is not installed; DEC-0021 requires Windows publication/recovery runtime evidence before target support is claimed. Current C-04 focused checks are green: embedding-table Arrow 14/14, row-link Arrow 15/15, embedding-table Parquet 10/10, row-link Parquet 12/12, artifact graph 25/25, embedding package 25/25, project package 41/41, focused warnings-denied Clippy, warnings-denied missing-docs builds, no-default embedding tests, CSV-only and Parquet-only all-target checks, locked standalone-fuzz compilation, and a 5,000-run seven-route structured fuzz execution. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- C-04 remains incomplete. Criterion/DHAT implementation and evidence, the mandatory 10,000 × 1,280 smoke and 1,000,000 × 256 scale run, authorized 32-bundle Rust reconciliation, and the complete phase-boundary gates are not yet executed or verified.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Current checkpoint scope

- The focused checkpoint adds status-safe borrowed row blocks, one constant-state QC/logical-digest accumulator shared by all constructors and scans, materialized block scans, bounded borrowed/managed Arrow and Parquet physical scans, and unforgeable verified table/row-link receipts required by `CellEmbeddingArtifact`.
- Physical scans retain only one charged Arrow batch or copied Parquet row group plus bounded metadata; an 8,193 × 1,280 regression proves both scans succeed at exact retained limits that reject full materialization. Arrow table and row-link materializers now charge their coexisting stock-decoded batch.
- Exact source import, domain construction, Arrow, and Parquet paths share one all-present bit-level case; mixed four-status and 12-case generated physical differentials agree on rows, logical digest, and every QC field. Aggregate-only custom `Debug` implementations cannot reveal present values or private fillers.
- Direct row-link Arrow compression/variadic/table-heavy Message tests, direct row-link Parquet schema/metadata/codec/encoding/auxiliary/numeric-drift tests, selected-middle-row-group confinement, borrowed/managed hostile parity, and managed integrity precedence are green. The graph token now carries and enforces the provenance output dimension.

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
- Accepted `DEC-0019`: keep compatibility run outputs separate from schema-bound immutable project objects and portable store-relative locators; verify store-backed semantic inputs before cache lookup.
- Accepted `DEC-0020`: use exact `cap-std 4.0.3` for descriptor-relative local artifact confinement and hard-link/directory-sync publication; add no live cloud or columnar dependency to `marklab-project`.
- Accepted `DEC-0021`: keep Windows directory durability conditional on target compilation and runtime publication/recovery evidence; do not infer support from source review.
- Accepted `DEC-0022`–`DEC-0024`: introduce the layer-3 embedding owner and scoped verified IO, materialize expected/source-identity/spatial-context semantics, preflight every columnar input before stock decode, reconcile the real corpus without promotion, and explicitly defer OS mmap.
- Accepted `DEC-0025`–`DEC-0026`: freeze reconciliation-only identities and split source adaptation from provenance-gated finalization.
- Accepted `DEC-0027`: freeze exact Arrow/Parquet profile details and location-free fresh-artifact publication.
- Accepted `DEC-0028`: permit the independent fuzz lock to mirror only exact root-reviewed registry tuples while retaining every pre-C-04 identity tuple and disclosing changed dependency targets.
- Accepted `DEC-0029`: use the stable cached-metadata Parquet builder with exact row-group selection and a checked virtual row-group window after bounded raw validation; do not enable Parquet's broad experimental feature.
- Accepted `DEC-0030`: freeze block/scan exposure, benchmark profile selection, fixed arithmetic/order/checksum semantics, DHAT ownership, and the host RSS threshold before implementing scale evidence.

## Unresolved questions

- Thirty-two authorized Schürch NPY/CSV source bundles have explicit unique source-local identifiers and exact native/embedding row alignment, but no reviewed mapping into canonical typed `CellId`/hierarchy membership; three later aggregate arrays reuse source rows and are not canonical raw sources.
- The source code establishes CellViT-SAM-H layer-32 `z4` extraction and bounding-box token-mean pooling, but complete checkpoint-run input normalization/source-license provenance remains unavailable without a separately reviewed manifest.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Commit the independently reviewed streaming scan/QC and verified compact-artifact checkpoint without claiming C-04 closure.
2. Implement the frozen Criterion and DHAT workload, then execute the mandatory 10,000 × 1,280 smoke and heap gates.
3. Execute the mandatory 1,000,000 × 256 RSS-bounded run, authorized 32-bundle Rust reconciliation, and complete C-04 phase gates before evaluating closure.

Next exact verification command:

```bash
cargo +1.96.0 test --locked --features csv,parquet,dhat-heap --test cellvit_embedding_heap -- --exact embedding_10k_1280_peak
```
