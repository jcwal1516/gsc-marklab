# Implementation status

Last updated: 2026-08-24T01:10:25-04:00

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Committed baseline before the current checkpoint: `e0efb60a8110f606bc2964c5947e443c843f9394`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: Phase 2 / WS-C / C-06 first status-to-computation slice complete; broader FND-04/C-06 remains active

## Requirements

- Completed: `A-01`, `A-02`, `A-03`, `B-01`, `B-02`, `B-03`, `B-04`, `C-01`, `C-02`, `C-03`, `C-04`, `C-05`, `EMB-CORE`, `EMB-PATCH`, `FND-01`, `SLIDE-INV`, `WS-A`, `WS-B`, `WS-10`, `WS-20`, `WS-21`
- Active: `DATA-01`, `FND-04`, `FND-05`, `C-06`, `COH-01`, `PLAT-01`, `WF-01`, `WS-11`, `WS-12`, `WS-24`
- Planned next: next dependency-ordered vertical caller; no general mark/format/geometry infrastructure is pre-authorized

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all six archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. The Windows cross-target C-03 check also exits 101 before project compilation because `x86_64-pc-windows-msvc` is not installed; DEC-0021 requires Windows publication/recovery runtime evidence before target support is claimed. Current C-04 focused checks are green: the earlier domain/source/Arrow/Parquet/store/fuzz scopes plus the exact 10,000 × 1,280 Criterion smoke, exact DHAT heap gate, and exact 1,000,000 × 256 RSS-bounded scale run. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- C-04 is complete through `55d1c8b` and `handoffs/C-04.md`. The 612-test workspace gate, exact focused/docs/feature/Clippy/WSI/fuzz/dependency/heap/benchmark/synthetic/reconciliation gates, and independent closure reviews pass. Exact unpatched registry packaging and Windows runtime admission remain known external release/platform blockers, not hidden green gates.
- The bounded C-05 derived-region finalization/receipt milestone is green: the combined artifact-graph target passes 52/52, including seven deterministic finalizer and three physical receipt cases; `marklab-embeddings` passes 56 unit plus 6 domain tests. The final all-feature workspace suite passes with root 293 passed/21 documented ignores, the 52-test graph target, embeddings 56 unit plus 6 domain tests, and WSI 10 passed/1 public-oracle ignore. Warning-denied no-default package and all-target/all-feature workspace Clippy, no-default workspace compilation, strict package docs, formatting, and diff checks pass. One bounded design audit approved the candidate-only receipt boundary; the no-default gate exposed and verified the corrected physical-feature ownership of its private graph token.
- The bounded C-05 derived-slide milestone is green for both patch-sourced and region-sourced flows. The combined graph target passes 63/63, including eleven slide support/graph/finalization/receipt cases; `marklab-embeddings` passes 56 unit, 6 domain, and 1 hostile-row-link test. The final all-feature workspace suite passes with root 293 passed/21 documented ignores, WSI 10 passed/1 public-oracle ignore, and every remaining executed integration/package/doc test green. One independent review found a decoded-provenance cross-slide capability gap; its exact regression failed red and passes after source-bound fixed-size slide lineage is required before graph minting. Warning-denied Clippy, no-default compilation, strict docs, formatting, and diff checks pass.
- C-05 and `EMB-PATCH` are complete as synthetic, bounded data infrastructure. The existing embedding fuzz target now exercises every C-05 input family and completed 20,000 bounded runs without a crash. One declarative reference agrees with all three typed tables, scan partitions, both physical formats, and borrowed/managed validation. The checksum-pinned 10,000 × 1,024 / 100,000-edge smoke, zero-current-byte 180,729,962-byte-peak DHAT publication, and prebuilt 100,000 × 1,024 / 1,000,000-edge run at 1,311,342,592-byte RSS pass. The single all-feature workspace gate passes 846/846 with 23 skipped. Real-source promotion, source-component and coordinate correspondence, geometric region proof, and tissue-window claims remain prohibited.
- The first C-06 vertical slice is green without a MarkTable, format, receipt, validator framework, config/result/CLI change, or dependency. `MeasurementStatus` names the four master-plan states; exact C-05 provenance maps direct patch extraction to morphology prediction and deterministic region/slide aggregation to derived summary. The immediate public caller computes bounded fixed-order mean squared Euclidean distance across declared patch-overlap edges, excludes non-present endpoints, returns typed `InsufficientPairs` instead of NaN, and records table/support/overlap/provenance identities. Focused default and no-default tests pass 7/7 each; data passes 1/1; embeddings passes 56 unit + 6 domain + 1 hostile integration; relevant warning-denied Clippy, strict docs, formatting, and diff checks pass. The sole all-feature workspace Nextest gate passes 853/853 in 64.524 s with 23 skipped and one slow test. One independent review found no concrete issue.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Current checkpoint scope

- C-04 delivers exact cell-table/validity/expected/identity/context/link/provenance owners, bounded non-pickle source import, embedding-specific Arrow/Parquet validation/publication/scans, fuzz/differential/resource evidence, and pinned scale workloads.
- The authorized 32-bundle source profile reconciles exactly but remains aggregate-only and non-promotable because `canonical_identity_mapping`, `input_normalization_and_run_configuration`, `reviewed_source_snapshot`, and `license_record` are absent.
- C-05's logical, record, graph, receipt, physical, fuzz, differential, allocation, and scale contracts are complete. Distinct patch/region/slide tables, exact patch context/footprints/overlap, vector-free links, strict provenance, all eight Arrow/Parquet families, direct patch authority, deterministic weighted-region finalization, and both deterministic arithmetic-mean slide paths pass their bounded evidence. Source-component correspondence, coordinate-source correspondence, geometric region proof, real-corpus promotion, and embedding science are deliberately outside this completion claim.
- C-06 now has one immediately called measurement-origin slice and one observable descriptive computation over C-05 patch artifacts. It does not close FND-04/C-06 or EMB-01: current `Pattern` still lacks canonical `CellId` rows and explicit status/provenance input, while general mark kinds, units, thresholds, missingness, observation windows, spatial weights, nulls, inference, serialization, and real-source promotion remain absent.
- A later aggregate/header audit found candidate patch-feature matrices at widths 384 and 1,024 plus patch image/coordinate containers. It found no promotable patch/region/slide table or canonical link. The evidence has no pinned digest and no production source grammar; the validation ledger records the exact claim ceiling and one bounded recursive-key-output audit defect.

## Dirty files and reason

- C-06 first slice only: one data enum, one exact provenance mapping, one allocation-free bounded descriptive patch-overlap computation, focused integration tests, and implementation evidence. No physical format, receipt, validator framework, dependency, lockfile, source adapter, corpus mutation/promotion, CLI/config/result, general mark table, geometry/window/weights abstraction, inferential method, or biological claim changed.

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
- Accepted `DEC-0036` plus its finalization addendum: stage exact region authority, recompute with frozen scalar/order semantics and explicit resource limits, and mint a receipt only after full candidate-bound physical validation.
- Accepted `DEC-0037`: bind slide support authority to the verified lower table's owning slide, close both arithmetic-mean finalization paths, and reuse the existing slide physical profile for candidate-bound receipts.
- Accepted `DEC-0038`: freeze and close the shared-vector Criterion, DHAT, checksum, compile/RSS, and full-scale evidence without adding production benchmark hooks.
- Accepted `DEC-0039`: expose the minimum four-state measurement-origin contract only through an immediate bounded C-05 patch-overlap computation; defer the general MarkTable and every unsupported format/ontology/workflow surface.

## Unresolved questions

- Thirty-two authorized Schürch NPY/CSV source bundles have explicit unique source-local identifiers and exact native/embedding row alignment, but no reviewed mapping into canonical typed `CellId`/hierarchy membership; three later aggregate arrays reuse source rows and are not canonical raw sources.
- The source code establishes CellViT-SAM-H layer-32 `z4` extraction and bounding-box token-mean pooling, but complete checkpoint-run input normalization/source-license provenance remains unavailable without a separately reviewed manifest.
- Candidate patch vectors exist, but there is no reviewed canonical `PatchId` mapping, exact C-02 frame/transform, stride/overlap/receptive field, observation support, complete provenance, canonical patch table, or promotable real-corpus `CellPatchLink`.
- Region-path candidates are cell-row or unbound bundles, and the slide-path candidate is multirow; no admissible region/slide source profile exists.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Audit the dependency-ordered master-plan frontier for the next production/user caller, with FND-02 sampled-support/window truth and a versioned scalar-mark input boundary as candidates rather than assumptions.
2. Freeze one bounded behavior/data flow and its exact identities, undefined states, resource limits, compatibility surface, and claim ceiling.
3. Implement it red–green and add no general infrastructure until that same milestone consumes it.

Next exact verification command:

```bash
cargo +1.96.0 test --locked --test patch_overlap_embedding_dispersion
```
