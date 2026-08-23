# Implementation status

Last updated: 2026-08-23T02:54:10-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Committed baseline before the current checkpoint: `ff1561286115689f09cf23fbed58c056b3cf3b14`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-04 partial canonical Parquet table/row-link, managed-store, and fuzz checkpoint

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `C-01`, `C-02`, `C-03`, `FND-01`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`, `WS-20`, `WS-21`
- Active: `C-04`, `DATA-01`, `EMB-CORE`, `FND-05`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`, `WS-24`
- Planned next: `C-05`–`C-06`

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all five archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. The Windows cross-target C-03 check also exits 101 before project compilation because `x86_64-pc-windows-msvc` is not installed; DEC-0021 requires Windows publication/recovery runtime evidence before target support is claimed. Current C-04 focused checks are green: embedding-table Arrow 14/14, row-link Arrow 15/15, embedding-table Parquet 10/10, row-link Parquet 11/11, artifact graph 16/16, embedding package 19/19, project package 41/41, focused warnings-denied Clippy, warnings-denied missing-docs builds, no-default embedding tests, locked standalone-fuzz compilation, and a 5,000-run seven-route structured fuzz execution. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- C-04 remains incomplete. Streaming scan/QC, shared in-memory/NPY/CSV/Arrow/Parquet differential cases, direct row-link compression/variadic and table-heavy Message regressions, a selected middle-row-group confinement regression, Criterion/DHAT, mandatory 10k and 1M scale evidence, and phase-boundary gates are not yet implemented or verified.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Current checkpoint scope

- The focused checkpoint adds exact deterministic Parquet 2.0 table and row-link writers, bounded canonical compact-Thrift footer/page preflight before stock decode, one validated row-group windows, borrowed and managed full validation, two-pass fresh publication, root facades, structured fuzz routing, and adversarial/resource/determinism/property tests.
- Public Arrow batches and Parquet row groups are 8,192 rows; the internal Parquet write batch and page-row property are 1,024. Canonical writers are uncompressed PLAIN with only required RLE levels, exact sorted metadata, no Arrow schema hint/dictionaries/statistics/indexes/bloom filters, and pinned small/8,192-row SHA-256 goldens.
- `DEC-0029` authorizes the stable cached-metadata reader after raw validation. Each decode copies one validated contiguous row group into a checked `ChunkReader`; the virtual window rejects every request outside that row group. Raw row-link PLAIN IDs and optional levels are compared pagewise with the supplied logical link before stock allocation.
- The root lock adds only the exact Bytes/Parquet/Thrift direct edges to the local package. The standalone fuzz lock adds only exact root-reviewed registry tuples, including pinned `twox-hash 2.1.2`; Parquet's broad `experimental` feature remains disabled.

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

## Unresolved questions

- Thirty-two authorized Schürch NPY/CSV source bundles have explicit unique source-local identifiers and exact native/embedding row alignment, but no reviewed mapping into canonical typed `CellId`/hierarchy membership; three later aggregate arrays reuse source rows and are not canonical raw sources.
- The source code establishes CellViT-SAM-H layer-32 `z4` extraction and bounding-box token-mean pooling, but complete checkpoint-run input normalization/source-license provenance remains unavailable without a separately reviewed manifest.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Commit the independently reviewed bounded Parquet table/row-link/store/fuzz checkpoint without claiming C-04 closure.
2. Add streaming scan/QC plus the shared in-memory/NPY/CSV/Arrow/Parquet differential corpus and the remaining direct hostile/selected-middle-row-group regressions.
3. Add Criterion/DHAT evidence and execute the mandatory 10k/1M scale gates before evaluating C-04 closure.

Next exact verification command:

```bash
cargo +1.96.0 test --locked --offline --features parquet --test cell_embedding_parquet --test cell_embedding_row_link_parquet --test cellvit_embedding_artifact_graph
```
