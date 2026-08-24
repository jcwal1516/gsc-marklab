# Implementation status

Last updated: 2026-08-23T20:21:50-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Committed baseline before the current checkpoint: `18de846f736f16913727c37557c65707d05bfb14`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-05 behavior-first implementation after contract freeze

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `C-01`, `C-02`, `C-03`, `C-04`, `EMB-CORE`, `FND-01`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`, `WS-20`, `WS-21`
- Active: `C-05`, `EMB-PATCH`, `DATA-01`, `FND-05`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`, `WS-24`
- Planned next: `C-06`

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all six archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. The Windows cross-target C-03 check also exits 101 before project compilation because `x86_64-pc-windows-msvc` is not installed; DEC-0021 requires Windows publication/recovery runtime evidence before target support is claimed. Current C-04 focused checks are green: the earlier domain/source/Arrow/Parquet/store/fuzz scopes plus the exact 10,000 × 1,280 Criterion smoke, exact DHAT heap gate, and exact 1,000,000 × 256 RSS-bounded scale run. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- C-04 is complete through `55d1c8b` and `handoffs/C-04.md`. The 612-test workspace gate, exact focused/docs/feature/Clippy/WSI/fuzz/dependency/heap/benchmark/synthetic/reconciliation gates, and independent closure reviews pass. Exact unpatched registry packaging and Windows runtime admission remain known external release/platform blockers, not hidden green gates.
- The bounded C-05 direct-patch receipt checkpoint is green: the artifact-graph target passes 26/26, including 10 support/matrix receipt cases; the unchanged matrix behavior/hostile/resource targets pass 10/10, 3/3, and 3/3; strict package docs, no-default package Clippy, no-default workspace compilation, workspace warning-denied Clippy, and the complete all-feature workspace suite pass. The workspace root reports 293 passed and 21 documented ignores; `marklab-embeddings` reports 51 unit tests plus 6 domain tests. Region/slide support receipts, derived graphs/finalization, and source-component correspondence are intentionally not claimed.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Current checkpoint scope

- C-04 delivers exact cell-table/validity/expected/identity/context/link/provenance owners, bounded non-pickle source import, embedding-specific Arrow/Parquet validation/publication/scans, fuzz/differential/resource evidence, and pinned scale workloads.
- The authorized 32-bundle source profile reconciles exactly but remains aggregate-only and non-promotable because `canonical_identity_mapping`, `input_normalization_and_run_configuration`, `reviewed_source_snapshot`, and `license_record` are absent.
- C-05's approved logical and record substrate includes distinct expected patch/region/slide sets, exact patch context/footprints/overlap, three typed embedding tables, vector-free links, strict patch source-identity/normalization values, all four canonical support variants, deterministic weighted/arithmetic derivation contracts, both cell-link producer modes, the exhaustive patch-region assessment descriptor, all four strict multiscale provenance variants, and the eighteen-role direct-patch structural graph. All eight physical families have exact Arrow IPC/Parquet writers, bounded raw-before-stock and stock readers, deterministic publication, exact records, and borrowed/managed parity. Footprint, overlap, assignment, edge, patch-region, and direct patch support/table now have separate graph-bound runtime receipts; the cell-link halves also form one format-neutral paired receipt. Direct patch table finalization checks source-row PatchId/status equality before dimension/support/provenance/logical bindings and still does not prove opaque source-vector components. Region/slide support receipts, derived graphs/finalization, source-component correspondence, fuzz/scale evidence, and closure remain open.
- A later aggregate/header audit found candidate patch-feature matrices at widths 384 and 1,024 plus patch image/coordinate containers. It found no promotable patch/region/slide table or canonical link. The evidence has no pinned digest and no production source grammar; the validation ledger records the exact claim ceiling and one bounded recursive-key-output audit defect.

## Dirty files and reason

- C-05 direct-patch receipt checkpoint: additive `VerifiedPatchEmbeddingSupportArtifact` and `VerifiedPatchEmbeddingTableArtifact` capabilities, four Arrow/Parquet borrowed/managed receipt entry points, the missing provenance logical binding in the existing direct graph token, focused receipt/format/status/boundary/integrity/privacy regressions, exports, and implementation ledgers. No manifest, dependency, lockfile, generated file, source adapter, corpus mutation/promotion, derived graph, or scientific-result change is dirty.

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
- Accepted `DEC-0032`: distinct typed multiscale tables over a sealed private core; preserve C-04 behavior/goldens.
- Accepted `DEC-0033`: own exact patch support/overlap once; explicit grouped cell assignments and vector-free links; region overlap remains declared pending FND-02 geometry.
- Accepted `DEC-0034`: record candidate patch features as inventory-only evidence and require eight narrow C-05 Arrow/Parquet profile families without admitting a real source adapter.
- Accepted `DEC-0035`: bound deterministic cell-containment candidate work per pass; keep the simple four-bucket index until scale evidence justifies a more complex owner.

## Unresolved questions

- Thirty-two authorized Schürch NPY/CSV source bundles have explicit unique source-local identifiers and exact native/embedding row alignment, but no reviewed mapping into canonical typed `CellId`/hierarchy membership; three later aggregate arrays reuse source rows and are not canonical raw sources.
- The source code establishes CellViT-SAM-H layer-32 `z4` extraction and bounding-box token-mean pooling, but complete checkpoint-run input normalization/source-license provenance remains unavailable without a separately reviewed manifest.
- Candidate patch vectors exist, but there is no reviewed canonical `PatchId` mapping, exact C-02 frame/transform, stride/overlap/receptive field, observation support, complete provenance, canonical patch table, or promotable real-corpus `CellPatchLink`.
- Region-path candidates are cell-row or unbound bundles, and the slide-path candidate is multirow; no admissible region/slide source profile exists.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Commit the reviewed direct-patch support/table receipt checkpoint with a clean worktree.
2. Implement and verify the region-from-patches support receipt, derived-region graph, deterministic finalization, and region table receipt.
3. Extend the same dependency-ordered boundary to both slide variants, then add the frozen fuzz and scale evidence.

Next exact verification command:

```bash
cargo +1.96.0 test --locked --all-features --test multiscale_embedding_artifact_graph
```
