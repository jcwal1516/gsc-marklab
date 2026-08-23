# Implementation status

Last updated: 2026-08-23T05:35:06-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Committed baseline before the current checkpoint: `55d1c8b8a07b2dbf40f77b5590127456f9042295`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-05 contract activation after C-04 closure

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `C-01`, `C-02`, `C-03`, `C-04`, `EMB-CORE`, `FND-01`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`, `WS-20`, `WS-21`
- Active: `DATA-01`, `FND-05`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`, `WS-24`
- Planned next: `C-05`–`C-06`

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all six archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. The Windows cross-target C-03 check also exits 101 before project compilation because `x86_64-pc-windows-msvc` is not installed; DEC-0021 requires Windows publication/recovery runtime evidence before target support is claimed. Current C-04 focused checks are green: the earlier domain/source/Arrow/Parquet/store/fuzz scopes plus the exact 10,000 × 1,280 Criterion smoke, exact DHAT heap gate, and exact 1,000,000 × 256 RSS-bounded scale run. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- C-04 is complete through `55d1c8b` and `handoffs/C-04.md`. The 612-test workspace gate, exact focused/docs/feature/Clippy/WSI/fuzz/dependency/heap/benchmark/synthetic/reconciliation gates, and independent closure reviews pass. Exact unpatched registry packaging and Windows runtime admission remain known external release/platform blockers, not hidden green gates.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Current checkpoint scope

- C-04 delivers exact cell-table/validity/expected/identity/context/link/provenance owners, bounded non-pickle source import, embedding-specific Arrow/Parquet validation/publication/scans, fuzz/differential/resource evidence, and pinned scale workloads.
- The authorized 32-bundle source profile reconciles exactly but remains aggregate-only and non-promotable because `canonical_identity_mapping`, `input_normalization_and_run_configuration`, `reviewed_source_snapshot`, and `license_record` are absent.
- C-05 is next in dependency order. It must freeze patch/region/slide embedding tables and `CellPatchLink` semantics against the actual inventory without inventing vectors, overlap identity, or observation windows.

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
- Accepted `DEC-0031`: freeze deterministic fixture values, the observable 4,096-row random-access reduction, both profile goldens, and current-binary runtime RSS separately from cold compile/link memory.

## Unresolved questions

- Thirty-two authorized Schürch NPY/CSV source bundles have explicit unique source-local identifiers and exact native/embedding row alignment, but no reviewed mapping into canonical typed `CellId`/hierarchy membership; three later aggregate arrays reuse source rows and are not canonical raw sources.
- The source code establishes CellViT-SAM-H layer-32 `z4` extraction and bounding-box token-mean pooling, but complete checkpoint-run input normalization/source-license provenance remains unavailable without a separately reviewed manifest.
- Canonical patch embeddings and `CellPatchLink`; patch images/metadata exist but vectors/links were not found.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Commit the reviewed C-04 closure handoff and ledger/requirements/interface/claims updates.
2. Freeze the C-05 task contract against the authorized patch/region/slide inventory and C-04/C-02 owners.
3. Implement C-05 behavior-first without promoting absent patch vectors or inferring sampled observation windows.

Next exact verification command:

```bash
rg -n "C-05|CellPatchLink|patch embedding|sampled-patch" docs/implementation crates tests
```
