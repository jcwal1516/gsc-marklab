# Implementation status

Last updated: 2026-08-26

## Identity

- Plan: Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan, audit date 2026-08-22
- Plan SHA-256: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`
- Audited/pinned SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
- Committed baseline before the current checkpoint: `a1a335239c8bc8ca17afdd2f2f2865c03a1df71c`
- Branch: `branch/frontier-transformation`
- Worktree: `/Users/user/Bench/gsc-marklab` (primary checkout; no additional worktree)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)`
- Current phase/workstream/task: full pseudocode declaration checkpoint complete; bounded PLAT-DUR-01 durable replay closed; BACK-01/WS-13 continuation active

## Pseudocode documentation pack

- Installed the repository-local pseudocode documentation pack at working SHA `622e8d784c6b667201ad7216144d8f2a28e9a488` without changing scientific production behavior.
- Authority remains `MASTER_PLAN.md`, whose current SHA-256 is `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`. `PSEUDOCODE_AUTHORITY_AND_ERRATA.md` supplies subordinate normative corrections; the gap register prohibits improvising missing methods from adjacent pseudocode.
- Exact verification command: `python3 docs/implementation/verify_pseudocode_pack.py`.
- Exact verification output: `PSEUDOCODE PACK VERIFIED`; `full_sha256=c0508109be2a954502bb1b1989ee98936f1c1838bd25fd92fcb33f09054fda51`; `split_files=18`; `lines=6000`.
- Active-task pseudocode sources are shared substrate §§2.5–2.6 and unified execution/validation §§121–123 and 130–133. The concrete `PLAT-DUR-01` contract remains narrower: one durable native classical node, no generic executor, backend registry, new result family, or scientific method.
- Binding unresolved gaps include missing classical/stable method pseudocode, Bayesian nonparametric niches, cross-attention/transformer fusion, foundation-model admission, E(2)/SE(2)-equivariant models, general interval/coverage contracts, full pathology geometry/domains, and exact backend/oracle selections. None is authorized for implementation by this ingestion.
- No Cargo test, formatter, Clippy, build, benchmark, or scientific validation command was run for this documentation-only ingestion. Existing active production/test changes were left untouched.
- `git diff --cached --check` was run and reported the pack's preserved Markdown hard-break trailing spaces and split-boundary blank lines in `PSEUDOCODE_FULL.md`, `PSEUDOCODE_INDEX.md`, and exact split files. Those bytes were not normalized because doing so would invalidate the required full/split digests and byte-identical concatenation; no unrelated whitespace error was accepted as passing.

## Current program checkpoint

- `marklab classical` now delivers the complete first bounded classical spatial-pathology workflow: supported CSV/Parquet input, one canonical exact 2-D MultiPolygon window, one immediately consumed exact streaming point/boundary plan, standard-border homogeneous K/L, conditional homogeneous CSR over the whole location pattern, deterministic ERL inference, strict typed result/cache identities, project scheduler execution, and atomic result/manifest/report output.
- `PP-06A` and `NUL-01A` are complete. The broader `FND-02`, `FND-03`, `FND-06`, `FND-07`, `PP-01`, `WS-22`, `WS-30`, and `WS-31` requirements remain explicitly represented with their unimplemented signed-distance/compartment/shared-plan/design/provenance/correction/method/calibration/scale work; this checkpoint is not program completion.
- Focused final evidence is 12 domain + 3 workflow + 4 CLI tests, affected legacy analyze 1/1, and legacy geometry 12/12. Final warning-denied Clippy, formatting, no-default, strict docs, all eight feature-matrix builds, and the workspace suite pass; Nextest is 950/950 with 23 documented skips and one expected slow test.
- The first workspace attempt identified an invalid CLI cfg boundary in core adapter files. That finding was corrected without weakening the architecture test; the final focused architecture contract and full workspace run pass.
- No benchmark, fuzz, DHAT/RSS, packaging, dependency, remote, or release gate was run because this increment made no optimization, new physical parser format, dependency, packaging, remote, or release claim. Stable PP-01 scale/calibration/oracle evidence remains open in the tracker.
- Explicit user direction and `DEC-0054` reaffirm the master plan's integrated multi-backend strategy. Advanced methods may use established Python/R/Stan/GPU/specialized engines through pinned typed adapters; a Rust port requires measured scientific and operational evidence.
- Durable cross-process classical replay now has one canonical project head, append-only execution ledger, and existing content-addressed store owner. Its prerequisite is closed; `BACK-01` remains active and `WS-13` is promoted for the next multi-backend platform outcome.

## Requirements

- Completed additions at this checkpoint: `NUL-01A`, `PP-06A`; the exact bounded classical workflow is complete while its broader parent rows remain open.
- Completed bounded outcome: `PLAT-DUR-01` advances `FND-07`, `PLAT-01`, `WF-01`, `WS-11`, and `WS-12` through durable replayable classical project execution; broader PLAT-01/WF-01 composition remains active.
- Active continuation: `BACK-01`/`WS-13` under `DEC-0054`; no advanced algorithm is presumed to require a native Rust implementation.

## Command state

- Known failing commands/tests: clean unpatched `cargo +1.96.0 package --locked --workspace` creates all six archives but exits 101 while verifying data against the published pre-C-02 `marklab-core 0.1.0`. DEC-0018 makes this release-blocking until an authorized version/dependency-ordered publication boundary; ephemeral local patches verified every archive successfully and are not claimed equivalent to registry resolvability. The Windows cross-target C-03 check also exits 101 before project compilation because `x86_64-pc-windows-msvc` is not installed; DEC-0021 requires Windows publication/recovery runtime evidence before target support is claimed. Current C-04 focused checks are green: the earlier domain/source/Arrow/Parquet/store/fuzz scopes plus the exact 10,000 × 1,280 Criterion smoke, exact DHAT heap gate, and exact 1,000,000 × 256 RSS-bounded scale run. Historical red/green and harness failures are recorded in the validation ledger. The WS-B feature matrix still has the explicitly recorded narrow warnings and is not claimed warning-clean.
- C-04 is complete through `55d1c8b` and `handoffs/C-04.md`. The 612-test workspace gate, exact focused/docs/feature/Clippy/WSI/fuzz/dependency/heap/benchmark/synthetic/reconciliation gates, and independent closure reviews pass. Exact unpatched registry packaging and Windows runtime admission remain known external release/platform blockers, not hidden green gates.
- The bounded C-05 derived-region finalization/receipt milestone is green: the combined artifact-graph target passes 52/52, including seven deterministic finalizer and three physical receipt cases; `marklab-embeddings` passes 56 unit plus 6 domain tests. The final all-feature workspace suite passes with root 293 passed/21 documented ignores, the 52-test graph target, embeddings 56 unit plus 6 domain tests, and WSI 10 passed/1 public-oracle ignore. Warning-denied no-default package and all-target/all-feature workspace Clippy, no-default workspace compilation, strict package docs, formatting, and diff checks pass. One bounded design audit approved the candidate-only receipt boundary; the no-default gate exposed and verified the corrected physical-feature ownership of its private graph token.
- The bounded C-05 derived-slide milestone is green for both patch-sourced and region-sourced flows. The combined graph target passes 63/63, including eleven slide support/graph/finalization/receipt cases; `marklab-embeddings` passes 56 unit, 6 domain, and 1 hostile-row-link test. The final all-feature workspace suite passes with root 293 passed/21 documented ignores, WSI 10 passed/1 public-oracle ignore, and every remaining executed integration/package/doc test green. One independent review found a decoded-provenance cross-slide capability gap; its exact regression failed red and passes after source-bound fixed-size slide lineage is required before graph minting. Warning-denied Clippy, no-default compilation, strict docs, formatting, and diff checks pass.
- C-05 and `EMB-PATCH` are complete as synthetic, bounded data infrastructure. The existing embedding fuzz target now exercises every C-05 input family and completed 20,000 bounded runs without a crash. One declarative reference agrees with all three typed tables, scan partitions, both physical formats, and borrowed/managed validation. The checksum-pinned 10,000 × 1,024 / 100,000-edge smoke, zero-current-byte 180,729,962-byte-peak DHAT publication, and prebuilt 100,000 × 1,024 / 1,000,000-edge run at 1,311,342,592-byte RSS pass. The single all-feature workspace gate passes 846/846 with 23 skipped. Real-source promotion, source-component and coordinate correspondence, geometric region proof, and tissue-window claims remain prohibited.
- The first C-06 vertical slice is green without a MarkTable, format, receipt, validator framework, config/result/CLI change, or dependency. `MeasurementStatus` names the four master-plan states; exact C-05 provenance maps direct patch extraction to morphology prediction and deterministic region/slide aggregation to derived summary. The immediate public caller computes bounded fixed-order mean squared Euclidean distance across declared patch-overlap edges, excludes non-present endpoints, returns typed `InsufficientPairs` instead of NaN, and records table/support/overlap/provenance identities. Focused default and no-default tests pass 7/7 each; data passes 1/1; embeddings passes 56 unit + 6 domain + 1 hostile integration; relevant warning-denied Clippy, strict docs, formatting, and diff checks pass. The sole all-feature workspace Nextest gate passes 853/853 in 64.524 s with 23 skipped and one slow test. One independent review found no concrete issue.
- The second C-06 vertical slice is green without changing `Pattern`, PatternLoader, config 0.2, result 0.3, CLI, dependencies, or physical formats. `DeclaredScalarPatternInput` borrows the compatibility arrays while binding strictly ordered typed CellIds, owning slide, exact physical `[X,Y]` micrometre frame, non-missing binary/optional probability declarations, measurement status, exact provenance records, and threshold evidence. The direct engine and separate local-scheduler node call the unchanged marked computation; runtime results expose compact row/frame/declaration identity and truthful endpoint routing, while cached bytes remain exactly result 0.3. Focused input/workflow tests pass 9/9 and 8/8 in both default and no-default modes; 41 relevant compatibility tests, full warning-denied workspace Clippy, no-default workspace compilation, strict warning-denied root docs, formatting, and diff checks pass. The sole workspace Nextest gate passes 870/870 in 64.923 s with 23 skipped and one slow test. The single review's missing-runtime-identity finding was reproduced and fixed. The broader `-D missing-docs` root probe remains non-green on pre-existing compatibility API documentation and is not claimed as a gate.
- The third C-06 vertical slice is green and directly consumes the completed C-05 region-finalization flow. `patch_region_embedding_dispersion` validates exact source/link/candidate/graph/direct-provenance/derived-provenance bindings before a checked `relation_count * dimension` work cap, then reports declared-fraction-weighted patch distance from the materialized `f32` region means. It excludes non-present sources, retains zero vectors, returns typed `InsufficientContributors`, exposes exact identities plus `MorphologyPrediction`/`DerivedSummary`, and adds no format, receipt, validator, node, codec, dependency, CLI/config/result change, or geometry claim. The focused final cases pass 6/6, the full derived-region target passes 69/69, embeddings pass 56 unit + 6 domain + 1 hostile test, no-default/workspace Clippy/docs/format gates pass, and the final workspace Nextest gate passes 876/876 in 69.194 s with 23 skipped and one slow test. The sole review found an invalid bitwise axis-permutation claim; the contract/test now promise only true sign-flip invariance and explicitly preserve component-order rounding.
- The fourth C-06 vertical slice is green and compares two existing declared scheduler outputs without a new node, codec, result schema, format, receipt, validator, dependency, or generalized comparison layer. `compare_declared_marked_prepost` checks each available runtime binding and exact semantic mark identity/status/routing/threshold bits before delegating unchanged to `compare_marked_prepost`; it retains both declared identities, provenance-bearing mark-use summaries, and borrowed exact timepoints while allowing different rows/slides/frames/evidence chains and claiming no correspondence. Focused affected default tests pass 34/34, pre/post units pass 13/13, no-default declared targets pass 21/21, warning-denied Clippy/no-default/docs/format gates pass, and the final workspace Nextest gate passes 880/880 in 64.416 s with 23 skipped and one slow test. The sole review found public-wrapper binding and missing-timepoint visibility gaps; both were corrected. Arbitrary same-row/same-label numeric result substitution remains caller-asserted because the pre-existing public runtime fields and result 0.3 carry no private producer proof.
- The fifth C-06 vertical slice is green and adds an exact O(1) binary marked-row prevalence change over two existing declared scheduler outputs. `compare_declared_marked_prevalence` reuses the semantic gate, strengthens both declared comparison callers with exact count/canonical-prevalence bindings, reports signed post-minus-pre change only when both sides contain rows, returns typed `InsufficientCells` otherwise, and always uses binary counts even for probability-routed spectra. It borrows both runtime contexts and adds no node, codec, result/schema field, format, receipt, validator framework, dependency, randomization, or inference. The focused target passes 5/5 in default and no-default modes; affected default targets pass 26/26 plus 13 pre/post units; warning-denied focused/workspace Clippy, no-default workspace compilation, strict docs, and formatting pass. The final workspace Nextest gate passes 881/881 in 64.648 s with 23 skipped and one slow test. The sole review found eager subtraction on the unavailable path; an explicit branch now performs arithmetic only for two nonempty sides.
- The sixth C-06 vertical slice is green and directly compares both completed C-05 slide-finalization paths over one exact ancestral patch table. `slide_embedding_aggregation_path_discrepancy` uses the existing candidates, graphs, provenance, and region-table receipt to validate every path/common-lineage binding before the exact dimension cap, then reports fixed-order `f64` mean squared component discrepancy or typed `InsufficientComparablePaths`. The hand oracle is 0.625; positive zero, missingness, exact/one-short work, semantic drift, and same-slide foreign lineage are covered. Derived-slide tests pass 16/16, the combined graph 74/74, embeddings 56 unit + 6 domain + 1 hostile integration, no-default/docs/Clippy/format gates pass, and the sole post-fix workspace Nextest gate passes 886/886 in 64.649 s with 23 skipped and one slow test. The sole review found row-ID binding after the budget; row lookup/identity now precedes the cap while status/vector traversal remains behind it.
- The seventh C-06 vertical slice is green and joins the completed declared binary-mark boundary to the verified C-04 cell-embedding artifact through one immediate scientific caller. `declared_binary_cell_embedding_centroid_discrepancy` binds exact table/artifact QC, ordered CellIds, and a domain-separated CellId-bound binary-grouping digest before conservative `present_rows * D + D` work and exact two-accumulator byte caps. It reports the fixed-order `f64` mean squared component difference between present marked/unmarked centroids or typed `InsufficientGroups`, retains every status count plus declaration/physical/provenance identity, and never routes groups by optional probabilities. The focused target passes 4/4, the full affected CellViT graph target 29/29, scalar input 9/9, embeddings 56 unit + 6 domain + 1 hostile integration, no-default/docs/Clippy/format gates pass, and the sole post-fix workspace Nextest gate passes 890/890 in 64.580 s with 23 skipped and one slow test. The sole review found that declaration identity omitted exact binary row assignments; the local grouping digest and swapped-assignment regression close that gap.
- The eighth C-06 vertical slice is green and gives the already-declared dense probability modality one immediate WS-50 scientific consumer. `declared_probability_cell_embedding_cross_covariance_energy` binds the required probability declaration/values, exact table/artifact QC, every ordered CellId, and a domain-separated raw-`f32` probability-value digest before conservative `2 * present_rows * D + 2 * D` work and exact two-accumulator byte caps. It reports fixed-order two-pass population mean squared component cross-covariance energy, or distinct typed `InsufficientPresentRows`/`NoProbabilityVariation`, while binary rows remain contextual and unused. The 1,280-component hand oracle is 0.625; the focused target passes 5/5, the complete affected CellViT graph 34/34, scalar input 9/9, embeddings 56 unit + 6 domain + 1 hostile integration, no-default/docs/Clippy/format gates pass, and the sole workspace Nextest gate passes 895/895 in 64.693 s with 23 skipped and one slow test. The sole review found no concrete issue.
- The ninth C-06 vertical slice is green and makes the existing S7 centroid computation one observable project execution without changing its estimand or result 0.3. `DeclaredBinaryCellEmbeddingCentroidNode` revalidates the target project and exact S7 binding before registering only Pattern/declared references; `run_single_with_store` verifies the four-to-six exact mark/evidence and embedding semantic records before every miss or hit. Its private fixed 18-byte `MLCBCENT` codec carries only status/value and reattaches all counts and identities from the revalidated binding. Focused behavior passes 8/8; the full CellViT target 42/42; declared/project/result compatibility 37/37; project/workflow packages 42/42; embeddings 56 unit + 6 domain + 1 hostile integration; no-default/docs/workspace Clippy/format gates pass; and the sole workspace Nextest gate passes 903/903 in 64.749 s with 23 skipped and one slow test. The sole review found no concrete issue.
- The tenth C-06 vertical slice is green and gives the existing dense `Pattern::nucleus_area_um2` column one concrete continuous-morphology declaration and immediate WS-50 scientific caller. `declared_nucleus_area_cell_embedding_cross_covariance_energy` requires finite strictly positive areas, exact scalar/project provenance, verified table/artifact QC, every ordered CellId, and a domain-separated raw-area digest before conservative `2 * present_rows * D + 2 * D` work and exact two-accumulator byte caps. It reports fixed-order two-pass population mean squared component cross-covariance energy or distinct typed `InsufficientPresentRows`/`NoNucleusAreaVariation`, while binary and optional probability marks remain contextual and unused. The 1,280-component hand oracle is 62.5; focused behavior passes 7/7, the complete CellViT target 49/49, declared/result compatibility 30/30, embeddings 56 unit + 6 domain + 1 hostile integration, no-default/docs/workspace Clippy/format gates pass, and the sole workspace Nextest gate passes 911/911 in 65.167 s with 23 skipped and one slow test. The sole review found no concrete issue.
- The eleventh C-06 vertical slice is green and exercises the completed C-04→C-05 boundary through one immediate WS-50/WS-51 local-diversity caller. `contained_cell_patch_embedding_dispersion` retains the already-verified expected-cell ID/digest on `CellEmbeddingArtifact`, requires exact contained-shared link/input-graph/paired-physical-receipt and ordered CellId binding, then reports incidence-weighted mean squared component deviation from eligible patch-local present-cell centroids. Interpolation is rejected; non-present rows and patches with fewer than two present incidences are excluded; overlaps remain repeated incidences rather than independent patches. The 1,280-component overlapping-membership oracle is 0.5; focused behavior passes 7/7, the complete CellViT target 56/56, cell-patch graph 7/7, embeddings 57 unit + 6 domain + 1 hostile integration, no-default/docs/workspace Clippy/format gates pass, and the sole workspace Nextest gate passes 919/919 in 63.359 s with 23 skipped and one slow test. The sole review found no concrete issue.
- The twelfth C-06 vertical slice is green and directly contrasts the existing exact binary assignments with the concrete positive nucleus-area measurement. `declared_binary_group_nucleus_area_contrast` revalidates the target project and exact nucleus provenance, binds every ordered binary/area pair in one domain-separated digest, and reports fixed-order `f64` marked and unmarked means plus marked-minus-unmarked difference only when both groups exist. Optional probabilities remain contextual and unused; unavailable inputs retain only the nonempty mean. The hand oracle is marked 25, unmarked 12, difference 13; focused behavior passes 6/6, scalar input 9/9, the S10 nucleus-area filter 7/7, no-default/docs/workspace Clippy/format gates pass, and the sole workspace Nextest gate passes 925/925 in 63.813 s with 23 skipped and one slow test. The sole review found no concrete issue.
- The thirteenth C-06 vertical slice is green and composes the completed S12 scalar authority with the existing contained-shared link, managed graph, and paired physical receipt. `contained_patch_binary_nucleus_area_contrast` retains the exact whole-input contrast, binds owning slide and every ordered CellId, then reports the equal-patch mean of eligible within-patch marked-minus-unmarked area contrasts. The unequal-incidence oracle is 7, distinct from whole-input 8 and incidence-weighted 7.2; overlaps remain repeated non-independent incidences. Focused behavior passes 6/6, the complete affected CellViT target 62/62, S12 6/6, and cell-patch graph 7/7; no-default/docs/workspace Clippy/format gates pass, and the sole workspace Nextest gate passes 931/931 in 67.390 s with 23 skipped and one slow test. The sole review's feature-gate and non-discriminating-oracle findings were corrected without a second review.
- Confirmed available: `cargo-nextest`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `cargo-fuzz`, `ssh`, `scp`, `rsync`.
- Confirmed unavailable: local `markdownlint-cli2`, `actionlint`, and Gnuplot. Criterion used Plotters; no Markdown/workflow lint pass is claimed.

## Current checkpoint scope

- C-04 delivers exact cell-table/validity/expected/identity/context/link/provenance owners, bounded non-pickle source import, embedding-specific Arrow/Parquet validation/publication/scans, fuzz/differential/resource evidence, and pinned scale workloads.
- The authorized 32-bundle source profile reconciles exactly but remains aggregate-only and non-promotable because `canonical_identity_mapping`, `input_normalization_and_run_configuration`, `reviewed_source_snapshot`, and `license_record` are absent.
- C-05's logical, record, graph, receipt, physical, fuzz, differential, allocation, and scale contracts are complete. Distinct patch/region/slide tables, exact patch context/footprints/overlap, vector-free links, strict provenance, all eight Arrow/Parquet families, direct patch authority, deterministic weighted-region finalization, and both deterministic arithmetic-mean slide paths pass their bounded evidence. Source-component correspondence, coordinate-source correspondence, geometric region proof, real-corpus promotion, and embedding science are deliberately outside this completion claim.
- C-06 now has seven immediately called embedding computations, two concrete binary/nucleus-area computations, an observable declared scalar-pattern engine/project workflow, a dedicated project workflow for the exact binary-centroid question, a runtime-only declared pre/post workflow, and an exact descriptive binary-prevalence change over scheduler outputs. Three cell–embedding computations join exact binary assignments, dense probability values, or concrete positive nucleus area to a verified cell table; the whole-input and contained-patch contrasts join exact binary assignments directly to nucleus area; the contained-patch embedding computation joins the verified table to the same physical C-05 containment authority; the region and cross-slide-path computations consume exact C-05 finalizer outputs and lineage proofs. Existing values, graphs, and receipts are consumed directly rather than generalized. The compatibility `Pattern` and result 0.3 remain unchanged. FND-04/FND-05/C-06 and EMB-01 remain active because general marks, arbitrary units/modalities, missingness, source-anchor correspondence, real-source promotion, observation windows, spatial weights, nulls/inference, and durable result provenance remain absent.
- A later aggregate/header audit found candidate patch-feature matrices at widths 384 and 1,024 plus patch image/coordinate containers. It found no promotable patch/region/slide table or canonical link. The evidence has no pinned digest and no production source grammar; the validation ledger records the exact claim ceiling and one bounded recursive-key-output audit defect.

## Dirty files and reason

- C-06 contained-patch binary-group nucleus-area contrast only: one all-feature caller over the existing S12 result and existing contained-shared link/graph/receipt, focused root exports, six additions in the existing boundary fixture, and affected implementation records. No Arrow/Parquet implementation, physical format, graph, receipt, validator, shared traversal/statistics abstraction, interpolation semantics, workflow/node/codec, dependency/lock, source adapter/promotion, AnalysisEngine/CLI/config/result/Pattern/loader, observation window, spatial weight, randomization, inference, patient/specimen, segmentation-validation, or biological claim changed.

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
- Accepted `DEC-0040`: bind only the existing binary/optional-probability marked engine to typed CellId/frame/status/provenance/threshold identity and the existing local scheduler; retain exact config 0.2/result 0.3 behavior and defer every broader mark/file/schema surface.
- Accepted `DEC-0041`: consume the exact C-05 region-finalization flow with one bounded declared-fraction aggregation-dispersion computation; preserve fixed component order and defer every format/validator/geometry/general-statistics surface.
- Accepted `DEC-0042`: compare two existing declared marked scheduler outputs through the unchanged legacy comparator, retain both runtime identities/evidence chains/timepoints, reject semantic conflation, and add no durable result, receipt, node, codec, or generalized comparison infrastructure.
- Accepted `DEC-0043`: expose exact binary marked-row prevalence change through a separate runtime caller, preserve the public C-06-S4 wrapper and result 0.3, and reject the incomplete randomization-summary alternative.
- Accepted `DEC-0044`: compare both completed slide aggregation paths through their existing candidates, graphs, provenance, and region-table receipt; require exact common patch lineage and add no cross-path receipt, physical profile, validator, or general comparison abstraction.
- Accepted `DEC-0045`: join exact declared binary assignments to the verified cell-embedding artifact through one bounded centroid-discrepancy caller; retain a local grouping digest and add no general mark, physical, receipt, graph, validator, or spatial/inference infrastructure.
- Accepted `DEC-0046`: consume the existing dense probability modality through one bounded population cross-covariance-energy caller; retain raw ordered probability identity and add no scalar/embedding/general-statistics/physical/workflow infrastructure.
- Accepted `DEC-0047`: expose the exact S7 centroid question as one semantic-store-verified project execution through a private fixed 18-byte codec; retain the existing cache trust boundary and add no generalized codec/result/receipt infrastructure.
- Accepted `DEC-0048`: consume the existing dense nucleus-area compatibility column through one fixed square-micrometre morphology declaration and bounded population cross-covariance-energy caller; add no general continuous-mark, unit-registry, physical, workflow, or shared-statistics infrastructure.
- Accepted `DEC-0049`: retain the already-verified expected-cell identity and immediately join the C-04 cell artifact to the C-05 contained-shared link/graph/receipt through one bounded local-dispersion computation; exclude interpolation and add no physical or general-statistics infrastructure.
- Accepted `DEC-0050`: contrast the existing concrete positive nucleus-area measurement across exact binary groups with one paired-value digest, fixed-order means, and typed insufficient-group availability; add no general comparison, physical, workflow, or inferential infrastructure.
- Accepted `DEC-0051`: compose the exact S12 scalar authority with the existing contained-shared graph/receipt through one equal-patch local contrast; retain repeated-overlap limits and add no physical, shared-statistics, window, spatial, workflow, or inferential infrastructure.

## Unresolved questions

- Thirty-two authorized Schürch NPY/CSV source bundles have explicit unique source-local identifiers and exact native/embedding row alignment, but no reviewed mapping into canonical typed `CellId`/hierarchy membership; three later aggregate arrays reuse source rows and are not canonical raw sources.
- The source code establishes CellViT-SAM-H layer-32 `z4` extraction and bounding-box token-mean pooling, but complete checkpoint-run input normalization/source-license provenance remains unavailable without a separately reviewed manifest.
- Candidate patch vectors exist, but there is no reviewed canonical `PatchId` mapping, exact C-02 frame/transform, stride/overlap/receptive field, observation support, complete provenance, canonical patch table, or promotable real-corpus `CellPatchLink`.
- Region-path candidates are cell-row or unbound bundles, and the slide-path candidate is multirow; no admissible region/slide source profile exists.
- Exact sampled-patch observation windows/tissue masks; the full WSI is not the honest inference window.
- Seven-slide CPTAC advertised/accessibility discrepancy and the recorded adapter wrapper failure/promotion deviation.
- Authorized internal-crate versioning and dependency-ordered publication are required before the unpatched workspace package gate can become release-ready.

## Next three exact actions

1. Commit the completed contained-patch binary-group nucleus-area contrast as one cohesive local commit.
2. Pause with a clean worktree after step 6, as requested by the user.
3. On explicit continuation, audit dependency-ordered candidates and freeze only one bounded observable caller; do not generalize codecs, results, formats, validators, receipts, or abstractions ahead of it.

Next exact verification command:

```bash
cargo +1.96.0 test --locked --all-features --test cellvit_embedding_artifact_graph contained_patch_binary_nucleus_area
```

## Full-program cohort checkpoint — 2026-08-24

- The explicit full-program mandate pauses PLAT-DUR-01; its inherited production, test, decision, interface, and ownership hunks remain preserved and were not advanced.
- `marklab-cohort` now owns three native deterministic patient-level workflows: independent scalar label permutation with optional exact blocks, complete-pair scalar sign flips, and unblocked common-axis functional L2 permutation. CLI surfaces are `marklab cohort permutation`, `paired-permutation`, and `functional-permutation`; strict version-one JSON families leave result format 0.3 unchanged.
- Oracle evidence passed: scalar and paired hand calculations, exact functional L2 trapezoid 12.5, blocked/unblocked scalar reference, paired slow sign-flip reference, functional slow L2 reference, byte determinism, duplicate/confounded/incomplete-pair rejection, and common-axis rejection.
- Checkpoint commands passed: focused `rustfmt --check`; `cargo +1.96.0 clippy --locked --package marklab-cohort --all-targets -- -D warnings`; `cargo +1.96.0 check --locked --package marklab-cohort --no-default-features`; `cargo +1.96.0 test --locked --package marklab-cohort`; all three cohort CLI integration targets; and `git diff --check`.
- Workspace-wide Clippy, Nextest, doc, no-default, and full-workspace gates were not run because they would directly compile or execute the explicitly paused PLAT-DUR-01 tree. This checkpoint makes no workspace-wide green claim.
- The next dependency-valid outcome is Max-T multiple-endpoint patient permutation. No local commit was created because shared dirty documentation files contain inherited PLAT-DUR-01 hunks that must not be staged or committed.

## Full-program cohort checkpoint 2 — 2026-08-24

- Added three more runnable native patient workflows: single-step Max-T for complete endpoint families, exact linear/fixed-RBF MMD for complete fingerprints, and exact Euclidean energy distance. Shared whole-patient shuffling, stable Welch contrast, and complete-fingerprint validation now have one canonical owner each.
- Hand oracles passed for both Max-T effects/statistics, linear unbiased MMD-squared 11, and scalar Euclidean energy distance 5.5. Independent slow references passed for Max-T adjusted p-values/critical value, linear and RBF MMD statistics/p-values, and Euclidean energy statistic/p-value.
- Checkpoint commands passed: focused `rustfmt --check`; warning-denied `marklab-cohort` Clippy; package no-default check; all cohort unit/doc/differential tests; Max-T/MMD/energy CLI integrations; and `git diff --check`.
- Workspace-wide canonical gates remain unrun because they would directly compile or execute paused PLAT-DUR-01. No workspace-wide green claim is made. No commit was created because shared dirty documentation files contain inherited PLAT-DUR-01 hunks.
- The cohort CLI's single-file publication boundary was extracted into a focused module. The remaining adapter file is still large and is scheduled for further responsibility-based splitting as additional commands move into per-workflow modules.
- Next dependency-valid outcome: versioned spatial fingerprint construction, then equivalence and noninferiority.

## Full-program cohort checkpoint 3 — 2026-08-24

- Added canonical structured spatial fingerprint construction/distance, patient-effect TOST equivalence, and directional patient-effect noninferiority. Fingerprint output retains named axes, values, uncertainties, weights, fixed missing/normalization policies, SHA-256 spec/content identities, and decomposed distance contributions.
- Independent oracles passed: Python `hashlib` digest goldens; weighted curve-L2 hand result; R 4.5.2 Student-t TOST SE/statistics/p-values/interval; and R 4.5.2 higher/lower noninferiority statistic/p-value/bounds. `statrs` 0.18.0 (MIT) is the locked production Student-t dependency; SciPy/statsmodels were unavailable locally and are not claimed.
- Checkpoint commands passed: focused `rustfmt --check`; warning-denied cohort Clippy; package no-default check; all cohort unit/doc/reference tests; fingerprint/equivalence/noninferiority CLI integrations; and `git diff --check`.
- Workspace-wide canonical gates remain unrun because they would directly compile or execute paused PLAT-DUR-01. No workspace-wide green claim is made. No commit was created because shared dirty documentation files contain inherited PLAT-DUR-01 hunks.
- Cohort CLI publication, effect-table input, equivalence, noninferiority, and fingerprint responsibilities now have focused child modules. Remaining large adapter code will continue moving behind per-workflow modules only when its immediate workflow is edited.
- Next dependency-valid outcomes: hierarchical bootstrap and remaining general cohort-design functions, then Bayesian model IR and backend lifecycle.

## Full-program Bayesian checkpoint 1 — 2026-08-24

- Added three runnable experimental PyMC 6.3.0 workflows: conjugate normal-mean NUTS, Gaussian patient varying-intercept partial pooling, and one-covariate random-effects site meta-regression with new-site prediction. Python 3.12 and all 29 resolved packages are retained in the repository `uv.lock`; each static worker and request is SHA-256 bound.
- Oracle evidence passed: analytic `Normal(2, sqrt(0.2))` posterior; byte-identical seeded rerun and forced typed nonconvergence; six-patient global/heterogeneity recovery with extreme-group shrinkage; and eight-site intercept/slope/heterogeneity recovery with new-site prediction. The initially centered meta-analysis fit was rejected for 13,621 divergent tree events and replaced by exact likelihood marginalization plus conditional latent-effect reconstruction; the final fixture has zero divergences.
- The Bayesian lifecycle now reports prior/posterior finiteness, constraint/identifiability state, rank R-hat, bulk/tail ESS, mean/SD MCSE, minimum chain E-BFMI, divergences, maximum-depth hits, and workflow-specific posterior-predictive distributions. Complete fits remain experimental; nonconverged fits are diagnostic-only.
- Focused checkpoint evidence: warning-denied `marklab-bayes` Clippy, package no-default check, 7 unit/doc/contract tests, all three live-backend CLI integrations, Python worker syntax, `uv lock --check`, and `git diff --check`. Workspace-wide gates remain intentionally unrun because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made.
- No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks. Next dependency-valid outcome: exact Gaussian-process regression through the same pinned PyMC environment.

## Full-program Bayesian checkpoint 2 — 2026-08-24

- Added exact one-dimensional micrometre Matérn-3/2 GP regression, identifiable one-factor/two-output exact coregionalization, and approximate-only VFE inducing-point GP. All own strict typed inputs/results, static PyMC 6.3.0 workers, exact environment/worker/request/data identities, explicit physical/kernel/noise/jitter conventions, and bounded dense or variational work.
- Evidence passed: three smooth-field exact interpolation targets; positive cross-output loading plus joint predictions; two-start ELBO/tail/prediction stability; and direct variational-versus-exact prediction RMSE <=0.35. Initial exact-GP adaptation with 353 divergences and initial multi-output inference with 535 depth hits were rejected and corrected without weakening diagnostic gates.
- The variational result is never called complete: stable output is `approximate_only`, unstable diagnostics are `nonconverged`, and unavailable gradient/importance diagnostics are explicit. No sparse-scale, tissue, calibration, biological, or clinical claim is made.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; package unit/doc tests; exact, multi-output, and variational/exact CLI integrations; Python worker syntax; locked `uv` resolution; and `git diff --check`. Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01.
- No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks. Next dependency-valid functions: low-rank predictive process, then NNGP plan/log-density.

## Full-program Bayesian checkpoint 3 — 2026-08-24

- Added four runnable native spatial foundations: bounded low-rank predictive-process variance diagnostics, physical-order NNGP plan/log density with a full-GP oracle, deterministic validated sparse weights, and proper/intrinsic CAR field density with explicit constrained normalization and island policy.
- Oracle evidence passed: predictive-process knot/interior residuals and diagonal restoration; all-predecessor NNGP/full-GP agreement; weights symmetry/normalization/components/island/digest determinism; and three-node proper/intrinsic CAR log densities `-1.8779553444319261` and `-0.9506058139671261`. Singular proper precision, intrinsic reject/exclude islands, and the CAR edge bound have focused regressions.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 16 package unit/doc tests; and the predictive-process, NNGP, spatial-weight, and CAR CLI integrations. No Python worker or lock changed after checkpoint 2, so worker syntax and `uv lock --check` were not rerun.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid functions: general constrained GMRF density, then SAR likelihood. SPDE remains blocked on an exact promoted 2-D window/mesh contract and is not conflated with these graph workflows.

## Full-program Bayesian checkpoint 4 — 2026-08-24

- Added general constrained GMRF density, a fixed-parameter Gaussian SAR lag/error likelihood, and a typed fitted SAR lifecycle through the pinned PyMC 6.3.0/Python 3.12 backend. GMRF normalization uses an explicit orthonormal null-space basis; fixed and fitted SAR share the same transformation/Jacobian semantics.
- Oracle evidence passed: projected `diag(2,3,4)` GMRF log determinant/quadratic/density; two-region lag/error SAR likelihoods and `0.8/0.2/1.0` descriptive impacts; partial-pivot inverse; and an eight-region lag recovery with positive slope/rho, finite PPC, zero divergences, and zero depth hits. The error branch on lag-generated data executes separately and remains truthfully `nonconverged` when bulk ESS misses 400, with diagnostic-only claims.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 24 package unit/doc tests; GMRF, fixed SAR, and live fitted SAR CLI integrations; new-worker Python syntax; `uv lock --check` resolving the unchanged 29-package lock; and `git diff --check`.
- The first combined syntax/lock command used `workers/python/marklab_pymc_sar_worker.py` while already inside `workers/python` and failed with `FileNotFoundError`; rerunning with local `marklab_pymc_sar_worker.py` passed, after which the lock and diff checks passed. No weaker substitute is claimed.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcomes: BYM disease-mapping likelihood/model, then scaled-ICAR BYM2. SPDE remains blocked on its exact 2-D window/mesh prerequisite.

## Full-program Bayesian checkpoint 5 — 2026-08-24

- Added exact-constraint Poisson BYM, scaled-ICAR BYM2, and one exact one-dimensional Matérn spatially varying coefficient model. Native ICAR construction now canonicalizes components, whitens the constrained Laplacian, and records original/unit-typical generalized marginal variance; the varying-coefficient workflow likewise uses an exact orthonormal sum-zero field.
- Synthetic evidence passed: twelve-region BYM positive-effect/separate-field/risk recovery; BYM2 sigma/phi and unit-typical scaling; and twelve-coordinate positive global/spatial effects with centered nonconstant varying field. All three live fits have zero divergences and zero maximum-depth hits.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 26 package unit/doc tests; BYM/BYM2/spatial-coefficient live CLI integrations; syntax checks for all three new workers; unchanged 29-package `uv lock --check`; and `git diff --check`.
- One compile-only command initially repeated Cargo's `--no-run` flag and failed argument parsing; the corrected single `--no-run` command compiled both BYM integration targets. No test or weaker substitute was claimed for the failed invocation.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid Bayesian function: annealed sequential Monte Carlo on a typed model with particle/temperature/evidence/ESS ancestry diagnostics. SPDE remains paused on its mesh prerequisite.

## Full-program Bayesian checkpoint 6 — 2026-08-24

- Added three inference-engine workflows: source-audited annealed PyMC SMC with full stage/ancestry/evidence state, exact-gradient scalar Poisson log-rate Laplace, and a bounded INLA-style Poisson-lognormal nested Laplace grid compared with the same model under noncentered PyMC NUTS. Laplace and nested Laplace remain explicitly `approximate_only`; no exact-posterior claim is made.
- Evidence passed: byte-deterministic SMC ancestry with analytic `Normal(2,sqrt(.2))` posterior and log evidence `-9.48047308903574`; scalar Laplace mode `0.6282607821567117`, Hessian `10.371739217843288`, and analytic variance/rate summaries; and five-region 81-point nested-Laplace normalization, low endpoint masses, latent-mean RMSE <=0.15 versus complete NUTS, zero divergences, and zero maximum-depth hits.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 27 package unit/doc tests; all three SMC/Laplace/INLA live CLI integrations; isolated syntax checks for all three workers; unchanged 29-package `uv lock --check`; and `git diff --check`.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is held-out-unit-aware PSIS-LOO, followed by compatible predictive model comparison. SPDE remains paused on its exact 2-D window/mesh prerequisite.

## Full-program Bayesian checkpoint 7 — 2026-08-24

- Added held-out-unit-aware PSIS-LOO through pinned ArviZ 1.3.0/arviz-stats 1.3.1, strict compatible pointwise predictive model comparison, and deterministic simulation-based calibration for the typed Normal-mean model against its exact conjugate independent posterior sampler. Comparison binds exact likelihood target, held-out unit, data/preprocessing identities, and unit IDs; high Pareto-k state always propagates.
- Evidence passed: conjugate four-patient PSIS ELPD agrees with exact leave-one-out prediction within 0.03; constructed Pareto shape >1 retains the exact refit/K-fold unit; a constant pointwise shift reports pairwise ELPD difference 6 and SE 0 while preprocessing drift is rejected; and two byte-identical 500-replicate SBC runs pass rank/ECDF, coverage, z-score, exact 0.8 shrinkage, and zero-failure gates.
- Affected checkpoint commands passed after findings were fixed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 28 package unit/doc tests; PSIS/model-comparison/SBC live CLI integrations; isolated syntax checks for both new workers; unchanged 29-package `uv lock --check`; and `git diff --check`.
- The first warning-denied Clippy run stopped on one `is_multiple_of` standard-library replacement and two nonminimal PSIS warning booleans. The mechanical Rust 1.96 fixes were applied and the complete formatting/Clippy/no-default/package-test chain then passed; no behavior or gate was weakened.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is typed prior sensitivity for the conjugate Normal model, followed by posterior predictive field summaries where current exact field contracts provide an honest immediate caller. SPDE remains paused on its exact 2-D window/mesh prerequisite.

## Full-program Bayesian checkpoint 8 — 2026-08-24

- Added exact conjugate Normal prior sensitivity across posterior, leave-one-out predictive, and declared decision quantities; an exact fixed rectangular log-linear inhomogeneous-Poisson likelihood with complete midpoint quadrature; and a fitted PyMC 6.3.0 NUTS lifecycle using that same event-sum minus grid-integral likelihood with cell intensity/residual/PPC output.
- Evidence passed: base `Normal(2,sqrt(.2))` posterior with skeptical-prior decision/material change; two-event constant-intensity event term `2ln2`, integral 4, and likelihood `2ln2-4`; incomplete-grid/constant-fit-covariate rejection; and four-cell fitted counts `[4,7,14,27]` recovering coefficient mean >0.5 with positive cell intensities/PPC and zero divergences/depth hits.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 31 package unit/doc tests; prior-sensitivity/fixed-IPP/fitted-IPP CLI integrations; isolated syntax checks for both workers; unchanged 29-package `uv lock --check`; and `git diff --check`.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is Berman–Turner quadrature/refinement evidence on the exact rectangle grid, followed by gridded LGCP construction. SPDE remains paused on its exact 2-D window/mesh prerequisite.

## Full-program Bayesian checkpoint 9 — 2026-08-24

- Added exact Berman–Turner observed/dummy cell-weight refinement, exact full-cell dense 2-D Matérn LGCP construction with deterministic Cholesky, and a fixed-hyperparameter noncentered PyMC 6.3.0 NUTS LGCP fit with per-cell posterior/residual and aggregate cell-count PPC summaries.
- Evidence passed: constant-intensity coarse/fine Berman–Turner objectives both equal `2ln2-4` with exact window weight; the 2x2 LGCP artifact conserves counts and has covariance diagonal 2.250001 with positive factorization; and the 3x3 fitted counts `[2,5,12]` per row recover coefficient mean >0.3, a nonconstant latent field, positive intensity/PPC, and zero divergences/depth hits.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 33 package unit/doc tests; all three Berman–Turner/LGCP-build/LGCP-fit CLI integrations; isolated syntax checks for the fitted LGCP and fitted IPP workers; unchanged 29-package `uv lock --check`; and `git diff --check`.
- The first warning-denied Clippy run stopped on two Berman–Turner `% != 0` divisibility checks. They were replaced with Rust 1.96 `is_multiple_of`, and the complete formatting/Clippy/no-default/package-test chain then passed without changing behavior or weakening a gate.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is an explicit bounded posterior-predictive replicated cell-pattern artifact using the fitted gridded LGCP lifecycle. SPDE remains paused on its exact 2-D window/mesh prerequisite.

## Full-program Bayesian checkpoint 10 — 2026-08-24

- Added complete-fit-only gridded-LGCP posterior pattern materialization with exact selected chain/draw provenance and piecewise-constant cell simulation; bounded Thomas simulation with a six-SD exact rounded-rectangle parent window and explicit Gaussian tail bound; and matched Matérn cluster simulation with exact bounded-radius parent expansion and uniform-disc offspring.
- Evidence passed: eight LGCP replicas conserve cell/point totals, keep every point in its exact cell, vary in total count, and are byte-identical across reruns; the 100x100 Thomas fixture conserves all latent-parent offspring and keeps parents/children in the declared dilation/window; and the matched Matérn fixture additionally proves every child-parent displacement is at most the declared radius. Both cluster simulators are byte-repeatable.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 35 package unit/doc tests; fitted-LGCP, predictive-LGCP, Thomas, and Matérn-cluster CLI integrations; isolated fitted-LGCP worker syntax; unchanged 29-package `uv lock --check`; and `git diff --check` plus new-file trailing-whitespace inspection.
- Direct pinned `rand` 0.8.6, `rand_chacha` 0.3.1, and `rand_distr` 0.4.3 edges were added to `marklab-bayes`; all three packages were already present at those exact versions in `Cargo.lock`, so no new third-party package or version entered the lock graph.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is bounded latent-parent cluster inference, followed by minimum-contrast cluster fitting. SPDE remains paused on its exact 2-D window/mesh prerequisite.

## Full-program Bayesian checkpoint 11 — 2026-08-25

- Recorded the latent-parent RJMCMC blocker after confirming local R 4.5.2 has no admitted point-process/RJ package and the pinned Python environment has no variable-dimension sampler; then completed independent Thomas K minimum contrast, exact Strauss sufficient-statistic/Papangelou mechanics, and bounded Strauss birth/death simulation.
- Evidence passed: an exact Thomas curve recovers kappa 0.002, sigma 10 um, and intensity-derived mean offspring 10 under caller-weighted, unit-weight, and interior-range fits with Rust-recomputed near-zero objective; the three-point Strauss hand oracle gives one interacting pair and proposal intensity 0.5; and a byte-repeatable 20,000-iteration gamma-one chain accepts births/deaths and stays on the Poisson expected-count scale while retaining all 20,001 states.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 36 package unit/doc tests; Thomas-contrast, Strauss-statistic, and Strauss-birth/death CLI integrations; isolated SciPy worker syntax; unchanged 29-package `uv lock --check`; and `git diff --check` plus new-file trailing-whitespace inspection.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is typed Gibbs pseudolikelihood, followed by the exchange-MCMC backend decision. SPDE and latent-parent RJMCMC retain their exact named blockers.

## Full-program Bayesian checkpoint 12 — 2026-08-25

- Added nested exact-grid Strauss pseudolikelihood with model/block-sandwich uncertainty, pointwise Geyer saturation, complete symmetric multitype Papangelou evaluation, and identified categorical/continuous joint location–mark model construction. Exchange MCMC was recorded blocked because the finite Strauss chain is not an exact/controlled auxiliary sampler.
- Evidence passed: coarse/fine Strauss tables each conserve 400 square micrometres and yield improved bounded fits with positive model/robust SEs; Geyer raw counts `[1,1,0]` saturate to two; typed pair factors 0.5 and 2 cancel around baseline intensity 2; categorical marks preserve reference-softmax/null-comparison semantics; and continuous marks preserve fixed-one/positive shared loadings plus required separate-model comparison.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 36 package unit/doc tests; all five live CLI integrations; isolated SciPy pseudolikelihood worker syntax; unchanged 29-package `uv lock --check`; and `git diff --check` plus new-file trailing-whitespace inspection.
- The initial continuous-mark run exposed inverted finiteness predicates, and the paired categorical regression caught the collateral predicate edit. Both exact validation defects were fixed and both integrations rerun together successfully.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is joint location–embedding latent-factor construction, followed by replicated hierarchical LGCP construction. SPDE, latent-parent RJMCMC, and exchange MCMC retain their named prerequisites.

## Full-program Bayesian checkpoint 13 — 2026-08-25

- Added rotationally identified joint location–embedding latent-factor construction, patient-nested replicated hierarchical LGCP construction with explicit field-sharing policy/no-concatenation semantics, and count/translation-K posterior-predictive diagnostics with standardized simultaneous max-deviation envelopes. Replicated cluster inference was recorded blocked on latent-parent or calibrated SBI infrastructure.
- Evidence passed: an eight-point three-dimensional/two-factor artifact preserves feature order, positive lower-triangular loading identification, physical Matérn semantics, shrinkage, and simpler comparators; three patients with two patterns each preserve all six window/grid/covariate identities under shared-plus-replicate fields; and two observed patterns/twenty replicas produce complete finite identical-estimator count/K diagnostics and simultaneous bounds.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 36 package unit/doc tests; all three live CLI integrations; unchanged 29-package `uv lock --check`; `git diff --check`; and new-file trailing-whitespace inspection rerun successfully from the repository root after an initial path-context mistake.
- The replicated-LGCP red run exposed an inverted SHA-invalidity predicate; it was corrected and the focused test rerun successfully. No assertion or gate was weakened.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Next dependency-valid outcome is vector semivariograms for spatial embeddings, followed by projected embedding variograms. SPDE, latent-parent, exchange, and replicated-cluster inference retain named prerequisites.

## Full-program embedding checkpoint 14 — 2026-08-25

- Added exact weighted omnibus vector semivariograms, training-only PCA projected component variograms with within-stratum complete-vector random labeling and single-step max-T, and globally centered/symmetrized embedding cross-covariance matrices with trace/Frobenius invariants plus a full matrix artifact.
- Evidence passed: the weighted two-bin hand oracle gives semivariances `2` and `8/3` and is unchanged by orthogonal rotation; an extreme held-out dimension cannot change the training center `[1.5,0]` or first-axis PCA loading and train/test observed projected curves agree; and the one-pair cross-covariance matrix equals `[[-8/9,2/9],[2/9,4/9]]` with trace `-4/9`, Frobenius `sqrt(88)/9`, and rotation-invariant summaries.
- The projected workflow uses pinned SciPy 1.18.1 from the unchanged 29-package Python 3.12 lock. Rust independently validates the returned training mean, covariance eigenproblem, orthonormal/sign convention, split pair counts, observed semivariograms, and max-T p-value lattice before publication.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 37 package unit/doc tests; all three live CLI integrations; unchanged `uv lock --check`; `git diff --check`; and new-file trailing-whitespace inspection.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- These are bounded synthetic owners for the three Part V pseudocode functions, not stable EMB-01/WS-50 promotion: canonical real embedding provenance, shared admitted geometry/edge correction, technical-confounder evidence, null calibration beyond the projected workflow, and patient-level validation remain open. Next dependency-valid outcome is cross-modal covariance by distance, then kernel mark correlation/global-envelope inference. SPDE, latent-parent, exchange, and replicated-cluster inference retain named prerequisites.

## Full-program embedding checkpoint 15 — 2026-08-25

- Added explicit-pair cross-modal covariance with source-section/compartment random labeling and bin-family max-T, training-only linear/cosine/RBF/Laplacian kernel construction with split-specific raw/normalized mark-correlation curves, and complete-vector within-stratum vector-semivariogram inference with a canonical ERL simultaneous envelope.
- Evidence passed: a four-pair 2-D A/B fixture returns near identity and far diagonal `[-1,1]` matrices with Frobenius `sqrt(2)` and max-T metadata; an RBF fixture freezes training center `[1.5,0]` and bandwidth `1.5` despite extreme held-out second coordinates and matches all hand kernel means/reference ratios; and an eight-object/two-stratum fixture conserves bin counts `2+7+19=28`, returns finite lattice-valued ERL inference, and is byte-identical on seeded rerun. The package ERL tie oracle matches the existing canonical depth `0.625` for four identical curves.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 38 package unit/doc tests; all three live CLI integrations; unchanged 29-package `uv lock --check`; `git diff --check`; and new-file trailing-whitespace inspection.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- These remain bounded synthetic owners, not stable EMB-01/MM-01/WS-50/WS-53 promotion. Canonical real multimodal correspondence/provenance, shared admitted geometry/edge correction, missing-modality policy, technical-confounder calibration, patient-level inference, and external validation remain open. Next dependency-valid outcome is graph Dirichlet energy and its complete-vector smoothness permutation test. SPDE, latent-parent, exchange, and replicated-cluster inference retain named prerequisites.

## Full-program embedding checkpoint 16 — 2026-08-25

- Added one exact symmetric weighted-graph owner consumed by combinatorial/symmetric-normalized vector Dirichlet energy, complete-row within-stratum SIGNAL-normalized smoothness permutation inference, and a node-keyed experimental local roughness artifact with explicit island status.
- Evidence passed: a weighted three-node two-component chain gives global numerator `41`, centered signal denominator `46/3`, and energy `123/46`; a six-node monotone two-component chain gives observed SIGNAL energy `2/7`, retains an inclusive lower-tail count exactly consistent with its plus-one p-value, and is byte-identical on seeded rerun; and the local chain-plus-island oracle returns `1`, `41/3`, `20`, and `0` with the island typed `isolated_node`.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default check; 38 package unit/doc tests; all three live CLI integrations; unchanged 29-package `uv lock --check`; `git diff --check`; and new-file trailing-whitespace inspection.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- These are bounded synthetic graph-signal owners, not stable GSP-01/EMB-01/WS-50/WS-62 promotion. Canonical FND-03 graph provenance/scale, real embedding admission, graph-selection/null calibration, multiplicity-controlled local inference, and patient-held-out validation remain open. Next dependency-valid outcome is explicit cell–patch linkage validation and weighted patch context. SPDE, latent-parent, exchange, and replicated-cluster inference retain named prerequisites.

## Full-program cell–patch checkpoint 17 — 2026-08-25

- Mapped pseudocode `ValidateCellPatchLinks` to the existing complete C-05 `CellPatchLink` construction/receipt owner instead of duplicating its link schema. Added public overlap-aware `cell_patch_context` and `patch_dependency_weighting` APIs in `marklab-embeddings`; the former consumes the latter immediately.
- Evidence passed: exact declared weights `[1/2,1/4,1/4]` over patch vectors `[0,0]`, `[4,0]`, `[0,8]` produce context `[1,2]`; overlap components `{a,b}`/`{c}` aggregate to weights `3/4` and `1/4`, yielding dependency-group count two and Kish effective count `1.6`. Exact table/link/overlap identities and six component operations are retained.
- Affected checkpoint commands passed: focused Rust formatting; warning-denied `marklab-embeddings` all-target Clippy; package no-default check; 22 package unit tests, 6 domain tests, package docs, and all 8 cell-patch link integrations. `git diff --check` and new-file trailing-whitespace inspection passed.
- Workspace-wide gates remain excluded because they would compile or execute paused PLAT-DUR-01; no workspace-wide claim is made. The focused embeddings commands necessarily compiled the shared `marklab-project` dependency but did not run, edit, or target PLAT-DUR-01 behavior. No commit was created because shared dirty documentation and root files retain inherited PLAT-DUR-01 hunks.
- Real cell/patch source correspondence and the nested patient-held-out outcome cohort remain missing, so FR-02/WS-51 stable promotion and real complementarity claims remain data-dependent. The next implementable workflow is synthetic nested patient-held-out `TestCellPatchComplementarity` through the pinned scientific Python environment; real claims will remain unavailable. SPDE, latent-parent, exchange, and replicated-cluster inference retain named prerequisites.

## Full-program embedding checkpoint 18 — 2026-08-25

- Added nested patient-held-out M0–M5 complementarity, a prespecified multiscale kernel with scale sensitivity, and exact training-only analogous-region retrieval. M0–M5 uses pinned SciPy 1.18.1 and the existing paired-patient inference owner; fingerprint construction/distance map to completed COH-FINGERPRINT-01.
- Evidence passed: all 24 complementarity patients receive one outer-held-out prediction, patch-bearing M2 has RMSE below `0.1` and one quarter of M0 with negative paired absolute-error increment; linear scale kernels `3/2` at weights `1/4,3/4` give `2.25` and drop-scale `2/3`; retrieval ranks `r2,r1`, decomposes squared distance exactly, reports exact recall one, and retains finite OOD score.
- Checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` Clippy; package no-default; 38 package unit/doc tests; all three live CLI integrations; unchanged 29-package `uv lock --check`; `git diff --check`; new-file trailing-whitespace inspection. Workspace-wide gates/commit remain excluded by paused inherited PLAT-DUR-01.
- Real matched outcomes/correspondence, external calibration, retrieval utility, and biological claims remain data-dependent. Next outcomes are patient-level embedding distribution comparison and within-patient region compatibility.

## Full-program prediction-safety checkpoint 19 — 2026-08-25

- Mapped patient embedding-distribution comparison to the completed whole-patient MMD/energy owners; added descriptive within-patient fingerprint compatibility with first-order endpoint-uncertainty propagation; and added training-fit/held-out-domain-threshold shrinkage Mahalanobis OOD plus prespecified abstention.
- Hand evidence gives region distance `2*sqrt(12.5)`, component/weighted-total standard uncertainty `0.1/0.2`; identity-covariance OOD validation scores `0,1,2` freeze threshold one and classify `sqrt(0.5)/sqrt(8)` as in/out; abstention retains equality and emits separate uncertainty/OOD reasons on strict exceedance.
- Checkpoint commands passed: `cargo +1.96.0 fmt --all --check`; warning-denied all-target Clippy for `marklab-cohort` and `marklab-bayes`; both package unit/reference/doc suites (39 Bayes unit and 18 cohort unit tests plus 9 cohort references); all three new CLI integrations; affected-package no-default check; `git diff --check`; and new-file trailing-whitespace inspection. Workspace-wide gates and a commit remain excluded because they would compile or mix the explicitly paused inherited PLAT-DUR-01 work.
- Real replicated region compatibility, patient-held-out calibration/utility, threshold transportability, and scientific or clinical claims remain data-dependent. The next independent predictive functions are probability calibration and grouped conformal prediction.

## Full-program calibrated-prediction checkpoint 20 — 2026-08-25

- Added pinned-SciPy patient-OOF Platt calibration, patient-level split-conformal binary prediction, and calibrated patient-OOF late fusion with explicit availability indicators, missingness scenarios, and modality ablations.
- Leakage/hand evidence passed: flipping all held-out calibration-test labels leaves Platt parameters/probabilities unchanged; conformal uses ten calibration patients and Rust replays corrected rank, sets, and site/subgroup coverage; late fusion consumes explicit OOF declarations, produces ten finite test probabilities across complete/two single-missing scenarios, two ablations, and Brier below `0.25`.
- Checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` all-target Clippy; package no-default; package unit/doc suite; all three live CLI integrations; unchanged pinned Python environment; `git diff --check`; and new-file trailing-whitespace inspection. Workspace-wide gates/commit remain excluded by paused inherited PLAT-DUR-01.
- Real probability calibration, exchangeability, conditional/shifted coverage, multimodal incremental value, and clinical utility remain data-dependent. Next runnable functions are mixture-of-experts fusion and grouped predictive stacking.

## Full-program multimodal-fusion checkpoint 21 — 2026-08-25

- Completed Part V with patient-grouped predictive stacking and context/availability mixture-of-experts fusion through pinned SciPy 1.18.1. Stacking retains exact jackknife sensitivity and forbids posterior-probability interpretation; expert gating excludes named technical shortcut contexts, regularizes collapse, calibrates separately, and reports context OOD.
- Hand/replay evidence passed: symmetric grouped densities optimize to exact `0.5/0.5`; a context-switching two-expert fixture favors the correct expert on each side, masks unavailable experts to `[0,1]`/`[1,0]`, and beats Brier `0.25`. Rust independently replays simplex mixtures/objective, gates, calibration, context OOD, Brier, and mean gate entropy.
- Checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` all-target Clippy; package no-default; package unit/doc suite; both live CLI integrations; unchanged pinned Python environment; `git diff --check`; and new-file trailing-whitespace inspection. Workspace-wide gates/commit remain excluded by paused inherited PLAT-DUR-01.
- Real multimodal predictive utility, exchangeability, site robustness, calibrated OOD, biological meaning, and clinical utility remain data-dependent. The next independent stream begins Part VI spatial omics and cross-modality integration.

## Full-program transport checkpoint 22 — 2026-08-25

- Added balanced log-domain Sinkhorn, KL-unbalanced log-domain Sinkhorn, and pinned-SciPy fixed-mass partial entropic transport. All bind exact supports/costs/resources, retain plan/objective/convergence artifacts, handle zero or unmatched mass explicitly, and prohibit correspondence claims.
- Closed-form evidence passed: symmetric two-cell balanced mass is `0.5/(1+exp(-1))`; one-cell unequal `2/8` mass with unit epsilon/KL penalties transports `16^(1/3)`; forced partial mass `1.5` from capacities `2/3` leaves `0.5/1.5` unmatched and costs six. Rust independently replays every partial constraint/objective.
- Checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` all-target Clippy; package no-default; package unit/doc suite; all three CLI integrations; unchanged 29-package Python lock; `git diff --check`; and new-file trailing-whitespace inspection. Workspace-wide gates/commit remain excluded by paused inherited PLAT-DUR-01.
- Registration/probabilistic correspondence remains blocked or data-dependent on uncertainty-bearing geometry, a validated nonrigid backend, and correspondence evidence. Next independent functions are dustbin soft assignment and fused Gromov–Wasserstein alignment/sensitivity.

## Full-program FGW checkpoint 23 — 2026-08-25

- Added explicit-dustbin entropic compatibility, pinned-POT 0.9.7.post1 balanced FGW with three initialization plans, and fixed-mass partial FGW with alpha/mass/epsilon/initialization sensitivity. All outputs retain scaled feature/structure objectives, convergence or feasibility, and non-correspondence claim ceilings.
- Regression evidence passed: a high real cost assigns over `0.99` mass to each unmatched state; a reversed-feature isometry receives over `0.9` balanced cross mass; forced partial mass `0.5` leaves `0.5/0.5` unmatched; and the independent-mass partial fit moves over `0.45` cross mass. The last case reproduces and guards against POT 0.9.7's scalar feature-gradient wrapper defect; Marklab instead uses POT's public log-domain entropic partial-Wasserstein subproblem with explicit FGW linearization and Rust replay.
- Checkpoint commands passed: focused Rust formatting; warning-denied `marklab-bayes` all-target Clippy; package unit/doc and no-default checks; all six transport/FGW CLI integrations; the 30-package Python lock check; two-worker syntax parsing; `git diff --check`; and new-file trailing-whitespace inspection. Workspace-wide gates/commit remain excluded by paused inherited PLAT-DUR-01.
- KL-unbalanced FGW is backend-blocked because POT exposes no FGW solver with the required KL-relaxed source/target marginals; unbalanced co-optimal transport is a different estimand. Atlas construction/query and correspondence claims remain data-dependent on admitted reference/query features, geometry, registration uncertainty, and external validation.

### Part VI atlas disposition

- `ATLAS-01` records `BuildSpatialAtlas`, `MapQueryToAtlas`, and `ValidateAtlasMapping` as one data-dependent workflow. No admitted reference/query cohort binds atlas supports, feature uncertainty, geometry, modality/model/stain provenance, and held-out perturbation labels.
- Existing exact analogous-region retrieval is not promoted into an atlas: it has no replayable serialized training payload, prototype variability, calibrated mapping probabilities, or segmentation/registration uncertainty propagation. The resume condition requires a provenance-complete cohort and held-out validation split.

### Part VII graph-mathematics disposition

- `GRAPH-MATH-01` records all Part VII graph/operator, Fourier, heat, wavelet, scattering, heterogeneous, hypergraph, motif, Hodge, and cellular-complex functions as blocked on the admitted WS-61 canonical graph contract and immediate scientific callers.
- Existing graph-signal energy/permutation/roughness owners retain their bounded synthetic scope. They do not establish coordinate/frame binding, construction scale, random-walk/operator semantics, or higher-order graph provenance, so they are not silently promoted into the canonical operator required by downstream mathematics.

### Part VIII topology/morphology disposition (superseded by checkpoints 40–41)

- `TOPOLOGY-01` originally recorded the missing backend/data prerequisites. IC-0158–IC-0164 now provide bounded experimental synthetic/supplied-input owners for all declarations while real pathology inputs, replication, and scale evidence remain absent.

### Part IX multimodal-model disposition

- `MULTIMODAL-MODEL-01` records all design-validation, pCCA, multiview/matrix/tensor/spatial-factor, missing-modality, joint-pathology fit/comparison, and validation-suite functions as data-dependent or backend-prerequisite-blocked.
- Existing joint model IRs and patient-OOF fusion/stacking/gating/covariance/complementarity workflows remain valid baselines but are not relabeled as general fitted latent multimodal models. Resume requires a matched measured cohort with exact entity joins, measurement status, missingness, technical/site variables, and patient-held-out outcomes.

## Full-program simulation milestone 24a — 2026-08-25

- Added the `marklab-simulation` owner and `marklab simulate growth-front` Fisher–KPP workflow. Exact logistic reaction splitting, centered no-flux diffusion, CFL enforcement, density invariants, cell-step bounds, mass/front trajectories, and the theoretical planar-wave control are explicit.
- Focused evidence passed: the closed-form uniform logistic state reaches exact `0.5`; a step front advances more than three micrometres while reporting theoretical speed one; and CFL one is rejected against the `0.5` limit. Formatting and warning-denied `marklab-simulation` Clippy pass, as do the package and three CLI tests.
- This ordinary milestone does not trigger workspace-wide gates or a commit, which remain excluded by paused inherited PLAT-DUR-01. General reaction–diffusion, pattern-instability diagnostics, competition, coupled mechanistic simulation, neural generators, SBI, and real biological validation remain open.

## Full-program simulation milestone 24b — 2026-08-25

- Added `marklab simulate spatial-competition` for two-species Lotka–Volterra density fields. Exact self-logistic and exponential cross/treatment flows are composed around dual no-flux CFL-bounded diffusion; output preserves states, mass/max trajectories, threshold events, exclusion status, and numerical invariants.
- Focused evidence passed: constant treatment `ln(2)` yields exact half-density after one time unit, while asymmetric competition `2/0.25` drives species A below `0.1`, retains B above `0.8`, and records A exclusion. Package formatting, tests, and warning-denied Clippy pass.
- Treatment remains a declared simulation input without a causal-effect claim. Stochastic agent competition is next; broader biological calibration remains open.

## Full-program simulation checkpoint 24 — 2026-08-25

- Added bounded stochastic `marklab simulate agent-competition` to the growth-front and density-field competition workflows. Named ChaCha20 replay, exact opposite-species neighborhoods, Gillespie event timing/selection, reflected coordinates, generated identities, full/truncated event accounting, and event/agent/pair bounds are explicit.
- Agent evidence passed: death-only seed 42 is byte-identical across reruns and removes the sole agent in one event; a zero-rate state terminates at time zero without inventing events. Together checkpoint 24 contains seven live CLI regressions across the three simulators.
- Stabilization commands passed: formatting; warning-denied `marklab-simulation` all-target Clippy; package unit/doc and no-default checks; all three simulator CLI suites; `git diff --check`; and new-file trailing-whitespace inspection. Workspace-wide gates and a commit remain excluded because they would include paused inherited PLAT-DUR-01.
- WS-70/WS-71 are now active rather than planned/blocked. General reaction-pattern, vascular/resource, interface/coupled, observation/noise, calibration, neural-generator, SBI, and real biological validation work remains open.

## Full-program simulation milestone 25a — 2026-08-25

- Added `marklab simulate reaction-diffusion`, a bounded scalar periodic 2-D specialization with typed exact linear/logistic reaction flows, explicit five-point diffusion, complete field/trajectory/work evidence, and centered Fourier pattern diagnostics.
- Focused evidence passed: exact linear doubling executes ten rather than floating-drift eleven steps and reports 40 cell-steps; a known alternating field reports wavelength two and unit nonzero spectral power; and one diffusion eigenmode step gives `1.096/0.904` at CFL `0.02`. A non-equilibrium reference explicitly suppresses linear-instability bands.
- The focused CLI suite, package tests/docs, warning-denied package Clippy, and formatting pass. This is the first checkpoint-25 workflow; arbitrary/vector PDEs, stochastic forcing, boundary/discretization sensitivity ensembles, and biological calibration remain open.

## Full-program simulation milestone 25b — 2026-08-25

- Added `marklab simulate evolve-interface`, a bounded regular 2-D level-set specialization with static spatial speed, curvature flow, Godunov upwinding, linear-extrapolation ghosts, planned time/work accounting, complete states, and interface-crossing trajectories.
- Focused evidence passed: a planar interface translates exactly one micrometre under unit speed in ten steps while positive curvature weight contributes zero planar curvature; a twice-scaled planar field reinitializes to exact signed distance without changing the recorded zero contour and reports its distance visits.
- The two CLI regressions, package tests/docs, warning-denied package Clippy, formatting, and diff whitespace pass. Time-dependent/coupled speed, higher-order schemes, fast marching, and biological calibration remain open; one resource/vessel workflow remains before checkpoint-25 stabilization.

## Full-program simulation checkpoint 25 — 2026-08-25

- Added `marklab simulate vascular-transport`, mapping declared vessel sources and cell uptake to bounded regular-grid variable diffusion/static flow. Exact local source/uptake splitting surrounds conservative no-flux diffusion/upwind advection; complete gradients, mass accounting, and four-neighbor hypoxic regions are retained.
- Vascular evidence passed: source two plus uptake one reaches concentration one at `ln(2)` with exact mass balance; a centered impulse conserves mass with the `0.96/0.01` stencil and four hypoxic corner regions; rightward flow maps `[1,0,0]` to `[0.9,0.1,0]` at CFL `0.1`.
- Together checkpoint 25 advances scalar periodic reaction–diffusion/pattern analysis, static-speed level-set evolution, and declared-flow vascular transport. Broader vector/mesh/boundary/coupled solvers and biological calibration remain open; the hierarchical distance-to-resource workflow is next.
- Stabilization passed: `cargo +1.96.0 fmt --all --check`; warning-denied all-target/all-feature `marklab-simulation` Clippy; package all-feature unit/doc and no-default checks; and all six simulator CLI suites (15 tests). Workspace-wide gates and a commit remain excluded because they would include paused inherited PLAT-DUR-01.

## Full-program resource-response milestone 26a — 2026-08-25

- Added `marklab bayes distance-to-resource`: exact unsigned point-to-segment geometry feeds a prespecified linear hinge spline with compartment, resource-density, accessibility, and proper patient random-intercept terms. Known-noise Gaussian inference is an exact bounded conjugate posterior, not an approximate backend fit.
- Focused synthetic evidence passed: twenty cells across four patients retain exact integer distances, recover distance/hinge coefficients two/three within `0.1`, achieve predictive RMSE below `0.05`, and report patient- plus nearest-resource predictive checks.
- The CLI regression, 39-test `marklab-bayes` package suite/docs, warning-denied package Clippy, formatting, and diff whitespace pass. Signed/network distances, inferred variance scales, non-Gaussian/GP response, real calibration, and causal claims remain open.

## Full-program mechanistic-coupling milestone 26b — 2026-08-25

- Added `marklab simulate mechanistic-tissue`, composing the existing vascular, scalar density, level-set, and agent owners over bounded aligned intervals. Oxygen saturation drives density growth and agent rates; local density/oxygen drives interface speed; aggregate work and mass diagnostics remain explicit.
- Focused evidence passed: mean oxygen one gives saturation one half, exact density `0.25→0.5`, one-micrometre planar interface translation, and one seeded hypoxia death. A second interval retains extinct agents without inventing events while field modules continue.
- The CLI regression, `marklab-simulation` package tests/docs, warning-denied package Clippy, formatting, and diff whitespace pass. Reciprocal/substep coupling, dynamic vessels, microscopy/segmentation noise, fitting, and digital-twin claims remain open.

## Full-program neural-generator admission audit — 2026-08-25

- `NEURAL-GEN-01` dispositions all eight neural Cox, neural marked, flow, and point-set diffusion pseudocode functions as blocked with exact backend/data prerequisites. The current lock has no reviewed pinned neural training/serialization/runtime, and no admitted independent-patient train/held-out pattern/context corpus carries required classical/LGCP calibration, privacy/memorization, or mode-collapse targets.
- No affine softplus helper, arbitrary ordering, or existing IPP/LGCP/fusion workflow is relabelled as a neural generator. Resume begins with neural Cox likelihood only after the named decision and data contract exist.
- Independent Part X implementation advances to differentiable spatial summaries and summary matching, which can be consumed immediately by SBI and simulator validation without a learned backend.

## Full-program simulation/SBI-summary checkpoint 26 — 2026-08-25

- Added `marklab simulate summary-matching`, implementing Gaussian soft pair-distance probability-density curves, analytic pair-distance derivative sums, exact pair-bin resource accounting, and a fully decomposed caller-weighted squared loss.
- Focused evidence passed: one centered pair matches `1/(h*sqrt(2*pi))`, a displaced generated pair has positive loss, and reversing an identical point set gives exact zero loss. Together checkpoint 26 contains exact hierarchical resource response, one-way mechanistic coupling, and differentiable summary matching.
- Neural Cox/marked/flow/diffusion functions remain explicitly blocked in `NEURAL-GEN-01`; these native summaries are not relabelled as learned generation or SBI calibration. Rejection ABC is next.
- Stabilization passed: formatting and diff whitespace; warning-denied all-target/all-feature Clippy for `marklab-bayes` and `marklab-simulation`; both packages' all-feature unit/doc and no-default checks; and all three checkpoint CLI regressions. Workspace-wide gates and a commit remain excluded because they would include paused inherited PLAT-DUR-01.

## Full-program SBI milestone 27a — 2026-08-25

- Added the dedicated `marklab-sbi` layer and `marklab bayes rejection-abc-growth-front`. Uniform-prior proposals use named ChaCha20 replay, every proposal calls the canonical simulator, final-mass discrepancies are explicitly scaled, and inclusive epsilon/acceptance/aggregate work accounting is retained.
- Focused evidence passed: thirty accepted draws recover the analytic logistic growth rate one within `0.03`, all distances satisfy epsilon one, proposals stay below 10,000, and repeated seed `20260825` is byte-identical.
- The CLI regression, SBI package unit/docs/no-default checks, warning-denied package Clippy, formatting, and diff whitespace pass. This is synthetic classical ABC, not calibrated biological inference; SMC-ABC is next.

## Full-program SBI milestone 27b — 2026-08-25

- Added `marklab bayes smc-abc-growth-front`, with strictly decreasing tolerances, weighted categorical ancestors, Gaussian perturbation, complete previous-mixture importance weights, normalized particles, ESS, adapted next-stage kernels, and bounded incomplete-stage failure.
- Focused evidence passed: 64 particles across epsilon `5,2,1` recover growth rate one within `0.03`; final weights sum to one within `1e-12`, every ESS exceeds one, and repeated seed `20260825` is byte-identical.
- The CLI regression, SBI package unit/docs/no-default checks, warning-denied package Clippy, formatting, and diff whitespace pass. Adaptive schedules and real calibration remain open; synthetic likelihood is next.

## Full-program SBI checkpoint 27 — 2026-08-25

- Added `marklab bayes synthetic-likelihood-growth-front`, estimating a two-summary Gaussian likelihood from canonical simulator replicates plus declared observation noise, with off-diagonal shrinkage, determinant/log-density/Monte-Carlo diagnostics, and retained-noisy-state random-walk MCMC.
- Focused evidence passed: 32 replicates and 1,500 iterations recover growth rate one within `0.05`, retain 1,000 post-burn draws, produce acceptance strictly between `0.05` and `0.95`, and replay byte-identically. Together checkpoint 27 covers rejection ABC, SMC-ABC, and synthetic likelihood/MCMC.
- Stabilization passed: formatting/diff whitespace; warning-denied all-target/all-feature SBI Clippy; SBI all-feature unit/doc and no-default checks; and all three SBI CLI regressions. Workspace-wide gates and a commit remain excluded because they would include paused inherited PLAT-DUR-01.

## Full-program learned-SBI admission audit — 2026-08-25

- `NEURAL-SBI-01` dispositions `TrainNPE`, `TrainNLE`, `TrainNRE`, and learned `SequentialSBI` as blocked on a reviewed pinned conditional-density/ratio backend plus an admitted calibrated simulation bank with round isolation, coverage/SBC, OOD, and proposal-budget contracts.
- Rejection ABC, SMC-ABC, and synthetic likelihood remain classical estimators and are not relabelled as neural amortized inference. No generic dispatcher or orphan encoder was added.
- Independent implementation advances to native `SimulationBasedCalibration` over the live analytic growth-front/rejection-ABC boundary.

## Full-program SBI calibration milestone 28a — 2026-08-25

- Added `marklab bayes growth-front-rejection-abc-sbc`, drawing prior truth, simulating canonical observed mass, rerunning rejection ABC under domain-separated seeds, and retaining complete ranks, equal-tail coverage, failures, proposals, and aggregate work.
- Focused evidence passed: 100 replicates have zero inference failures, mean normalized rank within `0.1` of one half, empirical 90% coverage in `[0.8,1]`, and byte-identical seed replay.
- The CLI regression, SBI package unit/docs/no-default checks, warning-denied package Clippy, formatting, and diff whitespace pass. This is one synthetic implementation-calibration control, not biological model validation.

## Full-program simulation-OOD milestone 28b — 2026-08-25

- Added `marklab bayes simulation-ood`: reference-only feature standardization, exact mean-kNN distance, held-out nearest-rank calibration threshold, conformal p-value, strict exceedance, exact split identities, and bounded feature-distance work.
- Focused evidence passed: `[10,10]` is out of support relative to a unit-square simulation bank, while calibration-like `[0.2,0.2]` remains in support under `k=2`, quantile `0.75`.
- The CLI regression, SBI package unit/docs/no-default checks, warning-denied package Clippy, formatting, and diff whitespace pass. Support is not model validity; learned/density/classifier and real-bank extensions remain open.

## Full-program SBI reliability checkpoint 28 — 2026-08-25

- Added `marklab bayes growth-front-posterior-predictive-lab`, with named posterior-draw replay, canonical simulator replicates, complete failures, and mass/maximum-density means, intervals, tail/two-sided discrepancy probabilities, and flags.
- Focused evidence passed: 100 replicates from growth rates `0.95,1,1.05` retain analytic rate-one observations inside both 90% intervals, emit no flags/failures, and replay byte-identically. Together checkpoint 28 covers SBC, simulation OOD, and posterior-predictive checks.
- Stabilization passed: formatting/diff whitespace; warning-denied all-target/all-feature SBI Clippy; SBI all-feature unit/doc and no-default checks; and all three checkpoint CLI regressions. Workspace-wide gates and a commit remain excluded because they would include paused inherited PLAT-DUR-01.

## Full-program longitudinal milestone 29a — 2026-08-25

- Extended `NEURAL-GEN-01` to disposition `ValidateGenerativeTissueModel`; Part X is now completely mapped, with learned validation still blocked on the same admitted backend/model/train/held-out evidence.
- Added `marklab-longitudinal` and `marklab longitudinal kalman-smooth`, implementing time-varying dense Kalman filtering, Gaussian innovation likelihoods, Joseph covariance updates, and Rauch–Tung–Striebel smoothing. Componentwise/all-missing observations, positive-semidefinite process noise, finite checks, and a prevalidated matrix-work ceiling are explicit.
- Focused evidence passed: exact scalar filtered means `1/2,1`, final variance `1/3`, first smoothed mean `1`; multivariate partial/all-missing updates; indefinite process-covariance rejection; and byte-identical CLI replay. Nonlinear/particle/spatial-field methods and biological longitudinal claims remain open.

## Full-program longitudinal milestone 29b — 2026-08-25

- Added `marklab longitudinal nonlinear-filter`, with caller-selected analytic quadratic EKF or scalar three-point UKF, explicit missing observations, Gaussian moment likelihoods, and complete derivative/sigma-spread/innovation/gain diagnostics.
- Focused evidence passed: both EKF and UKF reduce to the exact scalar linear filter (`1/2,1`, variance `1/3`) and replay byte-identically; the nonlinear missing-observation unit path performs prediction only. Package Clippy, package unit/docs, no-default check, formatting, and the CLI regression pass.
- This is a bounded scalar approximation, not nonlinear smoothing, a multivariate/non-Gaussian method, or biological validation. The third checkpoint-29 workflow is particle filtering/smoothing.

## Full-program longitudinal checkpoint 29 — 2026-08-25

- Added `marklab longitudinal particle-smooth`: bounded seeded scalar bootstrap propagation, stable sequential log weights, marginal-likelihood increments, ESS-triggered systematic resampling, complete particle/weight/ancestry histories, and terminal weighted ancestry-trace smoothing.
- Exact evidence passed: sixteen deterministic particles follow unit drift through filtering means and all four smoothed paths `[1,2,3]` with byte replay; a concentrated likelihood forces low ESS, resampling, valid ancestry, and uniform post-resampling weights.
- Together checkpoint 29 owns the Part XI linear Kalman/RTS, nonlinear EKF/UKF, particle filter, and ancestry-smoother functions through explicit finite specializations. Formatting/diff whitespace, warning-denied package Clippy, package unit/docs/no-default, and all three CLI suites pass. Workspace-wide gates and a commit remain excluded because they would include paused inherited PLAT-DUR-01.

## Full-program 3-D statistics milestone 30a — 2026-08-25

- Added `marklab-spatial3d` and `marklab spatial3d k-function`, jointly owning strict three-column physical-unit/voxel-spacing normalization, positive axis-aligned cuboid validation, optional SPD anisotropic distance, and homogeneous 3-D K/L with none, finite-sample border, or translation-volume correction.
- Focused evidence passed: two points one micrometre apart in a 10-cube produce K `1000`, `1000`, and `10000/9` at radius one for none/border/translation; millimetre input normalizes exactly to micrometres and replays byte-identically. Package tests prove anisotropy changes eligibility and reject non-SPD metrics.
- Formatting/diff whitespace, warning-denied package Clippy, package unit/docs/no-default, and the CLI regression pass. This is a retained-pair cuboid specialization with a one-million-pair hard cap, not general mesh/voxel geometry or real 3-D validation.

## Full-program 3-D statistics checkpoint 30 — 2026-08-25

- Added `marklab spatial3d inhomogeneous-k` and `cross-k` on IC-0126's exact normalized geometry. Per-point supplied intensities drive ordered inverse-intensity inhomogeneous K or directed A-to-B cross-K; cross-g uses successive 3-D shell volume. Border uses the exact eroded cuboid and eligible references, while translation retains exact overlap weighting.
- Focused evidence passed: two intensity-0.002 points in volume 1000 give inhomogeneous K 500; one A/one B point at intensity 0.001 give directed cross-K 1000 and positive shell g. A package oracle returns unavailable cross-g for a zero-volume first shell rather than NaN.
- Together checkpoint 30 owns `ValidateDimensionality`, a cuboid `ValidateWindow3D`, `KFunction3D`, `InhomogeneousK3D`, and `CrossK3D`. Formatting/diff whitespace, warning-denied package Clippy, package unit/docs/no-default, and both 3-D CLI suites pass. Workspace-wide gates and a commit remain excluded because paused inherited PLAT-DUR-01 is outside this work.

## Full-program 3-D graph milestone 31a — 2026-08-25

- Added `marklab spatial3d spatial-graph`, consuming IC-0126 geometry for physical radius or deterministic undirected union-kNN edges. Per-point radial uncertainty is handled only through named nominal, possible/lower-bound, or guaranteed/upper-bound distance; binary/Gaussian weights and symmetric CSR are complete.
- Focused evidence passed: x positions 0,1,3 yield only A–B under radius 1.5 and A–B/B–C under union-1NN; possible uncertainty admits an edge that guaranteed uncertainty excludes. Reversing point rows preserves the digest, while changing a graph-defining radius changes it even when edges remain identical.
- The direct `sha2 0.10.9` dependency reuses the existing lock and implements the recorded canonical digest decision. Formatting/diff whitespace, warning-denied package Clippy, package unit/docs/no-default, and all three 3-D CLI suites pass. This is descriptive bounded adjacency, not inferred registration uncertainty or biological interaction.

## Full-program evolutionary association milestone 31b — 2026-08-25

- Added `marklab longitudinal phylogenetic-spatial-association`: strict connected acyclic positive tree validation, unique clone-node mapping, canonical clone order, exact weighted path distances, physical 3-D centroid distances, and Pearson association over within-patient/specimen pairs only.
- The deterministic null independently permutes node assignments inside each biological block and retains every statistic with an inclusive plus-one two-sided p-value. A four-clone collinear path has correlation one, six pair rows, 199 null rows, p at most 0.2, and byte replay.
- Formatting/diff whitespace, warning-denied longitudinal Clippy, package unit/docs/no-default, and all four longitudinal CLI suites pass. The output explicitly prohibits migration-direction, ancestry, causal, or evolutionary-dynamics interpretation.

## Full-program Part XI checkpoint 31 — 2026-08-25

- `PART-XI-BLOCKERS-01` dispositions the residual serial-stack, general-window/alpha-complex, spatial-field, deformation/change, fitted-clone, and umbrella-suite functions against exact missing backend/data artifacts. Existing cuboid, graph, state-space, and association specializations are not relabelled as those broader families.
- `FitAnisotropic3DGP` remains planned work because the existing pinned PyMC lifecycle can be extended with a dedicated 3-D schema/worker and synthetic identifiability oracle; it is not falsely marked externally blocked.
- Together checkpoint 31 advances uncertainty-aware physical 3-D graph construction and block-restricted phylogenetic–spatial association while making every remaining Part XI dependency explicit. Scoped two-package stabilization follows; workspace gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program randomized interference milestone 32a — 2026-08-25

- Added `marklab-causal` and `marklab causal randomized-interference`. The workflow rejects unknown/nonconforming assignment, post-treatment baseline covariates, ineligible units, duplicate identities/edges, temporal reversal, and resource overflow before inference.
- Independent complete-randomization states are exactly enumerated by cluster. Binary-any-treated-neighbour exposures, exact joint probabilities, HT/Hájek means, positivity/denominator states, fixed-outcome rerandomization SDs, direct/spillover/total contrasts, and a full seeded HT null are retained.
- Evidence passed: a four-unit/two-treated line has six states and endpoint joint probabilities `1/6`; two independent two-unit clusters produce four Cartesian states; a post-treatment covariate fails; CLI output replays byte-identically. Package tests/docs, warning-denied Clippy, no-default, formatting, and diff whitespace pass. Claims remain randomized-design mechanics only.

## Full-program causal exposure milestone 32b — 2026-08-25

- Added `marklab causal exposure-mapping` with prespecified binary-any, treated-count, weighted-fraction, Gaussian physical-distance decay, multiscale cumulative count, and declared continuous-field branches. Unique graph/unit identity, positive weights/distances, branch-specific parameters, canonical unit order, isolate states, and exact visit bounds are enforced.
- One weighted three-node graph proves central-unit binary/count one, weighted fraction `2/3`, unit-bandwidth Gaussian exposure `exp(-1/2)`, multiscale `[0,1,1]`, and continuous field `0.2`. The randomized-interference CLI remains green after the package extension.
- Package tests/docs, both causal CLI suites, warning-denied Clippy, no-default, formatting, and diff whitespace pass. The mapping artifact contains no outcome and carries an exposure-construction-only ceiling.

## Full-program causal/design checkpoint 32 — 2026-08-25

- Added `marklab causal gaussian-eig`, a bounded scalar linear-Gaussian implementation of nested Monte Carlo expected information gain. Every outer truth/observation/log numerator/log denominator/value, mean, sample SE, analytic oracle, signed/absolute bias, seed namespace, and exact likelihood work is retained.
- Unit prior/noise/sensitivity gives analytic EIG `ln(2)/2`; 1,000 outer by 500 inner draws agree within four reported SE plus `0.05`, execute exactly 501,000 likelihood evaluations, and replay byte-identically. Zero sensitivity is rejected as an uninformative declared candidate.
- Together checkpoint 32 covers randomized design validation/interference inference, all six exposure-map branches, and analytic EIG validation. Package tests/docs, all three causal CLI suites, warning-denied Clippy, no-default, formatting, and diff whitespace pass. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program causal sensitivity checkpoint 33 — 2026-08-25

- Added `marklab causal bias-sensitivity`, retaining the complete binary-confounder assumption grid and adjusted-effect region. The hand grid yields biases `0.6,-0.6,3`, adjusted region `[-1,2.6]`, zero inclusion, and a sign reversal.
- Added `marklab causal manski-bounds`; treated/control means `0.7/0.3`, treatment fraction `0.5`, and support `[0,1]` produce potential-mean intervals `[0.35,0.85]`/`[0.15,0.65]` and ATE `[-0.3,0.7]`, rather than the observed `0.4` point contrast.
- Added `marklab causal rosenbaum-sign-sensitivity`; five positive pairs reproduce exact Gamma `1,2,4` sign-test bounds and critical Gamma one at alpha `0.05`. All six causal CLI suites, package tests/docs, warning-denied Clippy, no-default, formatting, and diff whitespace pass. None of these diagnostics corrects confounding or establishes an effect.

## Full-program Part XII prerequisite audit — 2026-08-25

- `PART-XII-BLOCKERS-01` dispositions every residual fitted observational, perturbational, mediation, sequential/active-selection, allocation/power, and umbrella-validation function against exact missing identification/data/action artifacts.
- IC-0130–0135 remain bounded synthetic/randomized/assumption-explicit mechanics. They are not promoted into propensity, DR/DML, mediation, prospective selection, or operational laboratory claims.
- Part XII function mapping is complete through live specializations or named resume conditions. Independent work advances to Part XIII; `PLAT-DUR-01` remains paused and untouched.

## Full-program stable numerics milestone 34a — 2026-08-25

- Added `marklab-numerics` and `marklab numerics stable-primitives`, centralizing max-shift/Neumaier log-sum/log-mean-exp, normalized compensated weighted mean, and two-pass symmetric covariance with unweighted `n-1` or weighted effective-sample denominator.
- Focused evidence passed: `[1000,999]` avoids exponential overflow; equal-weight `[1e16,1,-1e16]` returns `1/3` despite cancellation; `[1,2],[2,4],[3,6]` returns covariance `[[1,2],[2,4]]`; one effective weighted observation fails explicitly.
- Package tests/docs, warning-denied Clippy, no-default, CLI regression, formatting, and diff whitespace pass. No performance or fitted-model claim is made, and PLAT remains untouched.

## Full-program execution-policy checkpoint 34 — 2026-08-25

- Added `marklab-policy` with `marklab policy determine-maturity`. Closed maturity states only downgrade; all applicable reasons remain ordered, while incomplete provenance/nonconvergence/severe diagnostics/unsupported causal identification are terminal and approximation/predictive/Bayesian gaps impose softer caps.
- Added `marklab policy select-mode`. Every descriptor retains checked backend, memory, runtime, accuracy, and approximation-approval evidence. Explicit requests never fall back; automatic selection never silently approves approximation and may continue to a later feasible exact mode.
- Policy evidence passed: validated approximate predictive Bayesian input falls to research-only with three reasons; unsupported causal identification is terminal. An over-budget exact mode plus feasible approximation returns approval-required, then selects approximation only when approval is true. Package tests/docs, both policy CLI suites, warning-denied Clippy, no-default, formatting, and diff whitespace pass.
- Together checkpoint 34 owns four stable primitives, `DetermineResultMaturity`, and `SelectExecutionMode`. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program validation-ladder milestone 35a — 2026-08-25

- Added `marklab policy validation-ladder`, requiring exact ordered Stage 0–5 declarations, evidence for completion, and contiguous promotion. Completing synthetic/public/internal stages while leaving held-out/external/prospective incomplete promotes only to Stage 2 and retains all three later blocking risks.
- A package oracle rejects any completed stage after a gap. The first warning-denied Clippy pass found an obfuscated test conditional; the straightforward branch replacement passes package tests/docs, all three policy CLI suites, no-default, formatting, and diff whitespace.
- This is evidence bookkeeping, not evidence authentication or validation-stage achievement for any current scientific method.

## Full-program Part XIII prerequisite audit — 2026-08-25

- `PART-XIII-BLOCKERS-01` dispositions unified execution, deterministic parallel reduction, generic fitted diagnostics/calibration, and generic scaling benchmarks against their exact paused-platform/immediate-caller/schema/workload prerequisites.
- The audit does not touch or test PLAT-DUR-01 and does not relabel specialized diagnostics, calibration suites, or performance ledgers as generic framework completion.
- Part XIII function mapping is complete through stable numerics, maturity/mode/validation policies, or named resume conditions. Work advances to the 283-function coverage-index reconciliation.

## Full-program anisotropic 3-D GP milestone 35b — 2026-08-25

- Added `marklab bayes anisotropic-gp-3d`, implementing `FitAnisotropic3DGP` through the existing pinned PyMC 6.3.0 environment with three inferred axis length scales, strict diagnostics, bounded dense work, and conditional predictions.
- Independent Rust kernel oracles and the real 12-observation/two-prediction worker fixture pass; the result remains experimental axis-aligned interpolation.
- Part XI's residual blocker contract now distinguishes this completed function from still-blocked serial, general-window/topology, deformation, spatial-field, and fitted-clone workflows.

## Full-program cohort residual checkpoint 36 — 2026-08-25

- Added `marklab cohort repeated-freedman-lane`, preserving whole-subject residual exchangeability and recovering the four-subject common within-subject slope `2.1`.
- Added `functional-equivalence` and `bootstrap-equivalence`, using whole-patient simultaneous maximum-deviation bands and the existing patient-first hierarchical percentile interval respectively; both retain strict margin rationales and experimental coverage ceilings.
- Added `multisite-inference` with exact inverse-variance fixed pooling, scalar REML random effects, Q diagnostics, prediction intervals where applicable, and every leave-one-site-out refit.
- All 20 cohort package tests plus eight affected cohort CLI suites, warning-denied package Clippy, no-default compilation, and formatting passed. Spatial MMD is the canonical COH-MMD-01 patient fingerprint workflow, not a duplicate implementation.

## Full-program graph spectral milestone 37a — 2026-08-25

- Added `marklab-graph` and `marklab graph spectral`, jointly implementing canonical physical-radius graph construction, combinatorial Laplacian, deterministic exact graph Fourier coefficients, and declared band summaries.
- The three-node path returns eigenvalues `0,1,3`, exact middle-band energy `2`, zero low/high energy, canonical IDs/edges, a complete digest, and verified reconstruction.
- Package tests/docs, warning-denied Clippy, no-default compilation, CLI oracle, formatting, and diff whitespace pass. Part VII coverage moves from 31 blocked to four live/27 remaining.

## Full-program graph heat/wavelet checkpoint 37 — 2026-08-25

- Added `marklab graph heat`, implementing exact kernels, signal application, heat signatures, and declared-pair diffusion distances from the canonical eigensystem. Zero-time and positive-time identity/mass/smoothing oracles pass.
- Added `marklab graph wavelet`, implementing fixed exact spectral band-pass/low-pass transforms and energies; the lambda-one path fixture matches `2 exp(-2)`.
- All three graph CLI suites, package tests/docs, warning-denied Clippy, no-default compilation, formatting, and diff whitespace pass. Part VII now has ten live declarations and 21 remaining.

## Full-program graph-spectrum null milestone 37b — 2026-08-25

- Added `marklab graph spectrum-null` with exact node-stratum binding, complete-row restricted permutations, fixed-eigensystem band curves, canonical ERL global inference, and scalar low-band inference.
- Moved ERL from the embedding-only module into `marklab-numerics`; both embedding and graph regressions pass the identical-curve depth `0.625` oracle.
- Fixed near-zero Laplacian eigenvalue canonicalization after the first null run truthfully exposed exclusion of the constant mode from a nonnegative band. Four graph CLIs plus embedding-envelope regression, package tests, warning-denied Clippy, formatting, and diff whitespace pass.

## Full-program Part VII graph checkpoint 38 — 2026-08-25

- Added `marklab graph chebyshev-heat`, immediately consuming the generic Chebyshev recurrence and adaptive heat-order selection with reference-tail/grid evidence plus mandatory exact-signal comparison.
- The path lambda-one signal matches `exp(-1)[1,0,-1]` within `1e-8`; coefficients/order/interval/error evidence and matrix-vector work remain explicit and are not called a continuous certificate.
- The first Part VII checkpoint now has five user-visible workflows and 13 live declarations. FR-01/FR-01A/FR-01B, GSP-01, WS-61, and WS-62 are truthfully active rather than blocked/planned.

## Full-program Part VII completion checkpoint 39 — 2026-08-25

- Added exact-spectrum diffusion wavelets and fixed-kernel graph scattering with explicit signal-perturbation stability ratios, preserving dense bounded and synthetic claim ceilings.
- Added typed heterogeneous spatial-near messaging, normalized hypergraph signal mathematics, typed triangle motifs/nulls, and canonical dimension-two clique/Hodge decomposition/filtering.
- Added research-only interpreted cellular complexes with closed oriented boundaries and declared segmentation-perturbation comparison. Added `marklab graph validate`, which passes every exact fixture and radius sensitivity while reporting unsupported graph-rule, registration, sparse-scale, and GPU dimensions.
- All 31 Part VII pseudocode declarations now have consumed live specializations under IC-0145–IC-0157. Package unit/doc tests, all 13 graph CLI suites, warning-denied graph Clippy, no-default graph compilation, formatting, and diff whitespace pass. Workspace-wide gates and a commit remain excluded because they would include preserved paused PLAT-DUR-01 work.

## Full-program Part VIII topology checkpoint 40 — 2026-08-25

- Pinned GUDHI 3.13.0 for exact alpha and weak witness persistence, with Python 3.12, lock/worker hashes, squared-alpha conventions, essential-class policy, and the effective CGAL/GPLv3 alpha license recorded. Alpha, persistence, landscape, image, Euler, and witness hand oracles pass.
- Pinned scikit-image 0.26.0/SciPy 1.18.1 for supplied-raster area/Crofton/Euler/disk morphology and added bounded native finite-size connectivity curves. Single-pixel and three-point spanning oracles pass.
- Ten of 13 Part VIII declarations are live. Formatting, warning-denied topology Clippy, no-default/package tests, four CLI suites, Python syntax compilation, and diff whitespace pass. Workspace-wide gates and a commit remain excluded by preserved paused PLAT-DUR-01 work.

## Full-program Part VIII completion checkpoint 41 — 2026-08-25

- Added whole-patient GUDHI bottleneck distance-energy comparison with exact within-stratum label enumeration; the four-patient oracle returns distance `1.5`, energy `3`, and `p=1/3`.
- Added the declared-mask perturbation laboratory and topology validation ledger. The one-toggle fixture recomputes all five sensitivity families; nine suite controls pass while sparse-memory scaling is explicitly unverified.
- All 13 Part VIII declarations are live under IC-0158–IC-0164. Warning-denied topology Clippy, no-default/package/doc checks, all seven CLI suites, six worker syntax checks, formatting, and diff whitespace pass. Workspace-wide gates and a commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program Part IX multimodal checkpoint 42 — 2026-08-25

- Added paired measured-patient Gaussian pCCA with training-only standardization and deterministic EM; the synthetic shared direction passes correlation and held-out prediction gates.
- Pinned CCA-Zoo 3.0.0/NumPyro 0.21.0/JAX 0.11.1 for Bayesian pCCA. An initial target-0.9 run exposed one divergence; the retained zero-divergence gate passes at target 0.99 with draw-wise sign alignment.
- Pinned mofapy2 0.7.4 (LGPL-3.0) for multiview VI and structural/MAR masked-view prediction; eight held-out values pass the RMSE gate. Five Part IX declarations are live. Formatting, all three CLI suites, three worker syntax checks, and diff whitespace pass; broad gates/commit remain excluded by paused PLAT-DUR-01.

## Full-program Part IX multimodal checkpoint 43 — 2026-08-25

- Added exact patient→specimen→region→cell factor-graph compilation and pinned one-view MOFA matrix factorization; the hand graph and four masked rank-one matrix entries pass.
- Added bounded graph-Laplacian MAP/Laplace spatial matrix factors plus JAX/SciPy CP and Tucker factors. Smooth-matrix and both three-mode rank-one masked-recovery oracles pass with explicit approximate states.
- Ten of 18 Part IX declarations are live under IC-0165–IC-0171. Scoped checkpoint commands and their exact results are recorded in the validation ledger; workspace-wide gates/commit remain excluded by paused inherited PLAT-DUR-01.

## Full-program Part IX completion checkpoint 44 — 2026-08-25

- Added PyMC exact Matérn spatial factors, supplied-basis multiresolution factors, and anchor-preserving modality-dropout training; all three masked synthetic oracles pass.
- Added one consumed joint pathology compiler/fitter for five typed likelihood blocks and held-out clinical prediction, plus a multimodal alias for the canonical nested patient-held-out M0–M5 comparison.
- The validation suite passes eleven synthetic controls and explicitly reports SPDE mesh recovery, same-model inference-engine comparison, and admitted real-patient validation as unavailable. Seventeen of 18 Part IX declarations are live; only SPDE remains genuinely prerequisite-blocked.

## Full-program Part VI registration/atlas revisit checkpoint 45 — 2026-08-25

- Pinned stable SimpleITK 2.5.5 and delivered B-spline multiresolution registration. Added distinct JAX stationary-velocity scaling/squaring and landmark Hamiltonian LDDMM workflows.
- Added a calibrated variational translation-SVF posterior and an analytic GP landmark posterior consumed by Monte Carlo/delta propagation and uncertainty-averaged soft compatibility.
- Added synthetic patient-replicated biological atlas build/map/LOPO perturbation validation with OOD-unmatched mass and no physical-registration claim. All 19 Part VI declarations are live.

## Full-program Part X neural/generative revisit checkpoint 46 — 2026-08-25

- Added JAX marked Cox training with held-out likelihood/mark/quadrature evidence and an iid-equivariant logistic-normal point-set flow plus VP score-diffusion sampler.
- Pinned sbi 0.26.1/Torch 2.13.0 and trained distinct NPE/NLE/NRE estimands plus two proposal-isolated NPE rounds against an analytic posterior.
- Added a consumed repeated-artifact generative model card with held-out, diversity, memorization, membership-audit, baseline, and explicit unsupported/real-data rows. All 32 Part X declarations are live.

## Full-program Part XI advanced 3-D revisit checkpoint 47 — 2026-08-25

- Added paired-landmark serial reconstruction, Gaussian stack posterior, and transform-draw propagation into a 3-D endpoint; exact adjacent/reference composition and coverage controls pass.
- Added pinned GUDHI exact 3-D alpha complexes and a transform-posterior-conditioned deformation/biology model requiring negative control and independent measurement.
- Added uncertainty-integrated clone diffusion/niche fits and an advanced synthetic validation ledger. All 21 Part XI declarations are live; real registered longitudinal/clone validation remains absent.

## Full-program Part XII causal/active revisit checkpoint 48 — 2026-08-25

- Added a bounded synthetic observational laboratory with fitted propensity/overlap, dose response, cluster-cross-fitted AIPW and two-exposure DML, exposure effect, and adjusted negative-control result. Twelve disjoint clusters recover dose 2.0 and spillover 0.75 without a real-effect claim.
- Added a 16-cluster randomized perturbation workflow recovering controlled direct 1.0 and spillover 0.4. Its 1.2 path decomposition is explicitly research-only under DEC-0207 and cannot promote causal mediation.
- Added scalar-Gaussian sequential design, transparent constrained ROI/stain/landmark selection, biological-replicate allocation, seeded power surfaces, and a consumed validation ledger whose prospective laboratory row remains not verified. All 24 Part XII declarations are live; real identification and operational benefit remain absent.

## Full-program unified runtime and residual Bayesian checkpoint 49 — 2026-08-25

- The user explicitly unpaused the narrow PLAT-DUR-01 work required for full pseudocode completion. Extracted `marklab_workflow::execute_algorithm` from the proven durable restore/scheduler/commit path; `marklab project classical` is its immediate caller and all six cross-process/key-change/tamper/ledger/symlink focused tests pass.
- Added fixed-step HMC against an analytic normal posterior. Added a bounded advanced cluster flow with label-invariant parent-count birth/death inference, exact 64-state exchange MCMC, its consumed symmetric multitype fit, and partially pooled six-patient cluster parameters.
- Added fixed-contiguous threaded reduction consumed by typed HMC diagnostics and 4,000 Gaussian calibration fits, plus checksum-verified 1k/2k/4k equivalent-work smoke scaling. These are bounded synthetic/runtime mechanics, not general model validity or representative performance.

## Full-program terminal SPDE and declaration checkpoint 50 — 2026-08-25

- Added a shared pinned-SciPy rectangular finite-element owner: a 7x7 mesh has 49 vertices, 72 triangles, consistent mass/stiffness matrices, positive alpha-two Matérn precision, and barycentric projection row sums within `1e-12`.
- The same owner drives a fixed-hyperparameter LGCP MAP with count-conserving triangle quadrature and right-heavy intensity recovery, plus a one-factor spatial model with RMSE below 0.12 and x-correlation above 0.9 across two mesh resolutions.
- All 283 literal pseudocode declarations now have live consumed production specializations or canonical duplicate owners. Claim ceilings, registered authority/data gaps, and absent real/external/prospective evidence remain explicit; no commit, stage, push, deployment, history rewrite, or worktree was created.

## Full-program terminal verification checkpoint 51 — 2026-08-26

- Literal coverage is exact: 283 declarations and 283 live entries, with no missing or extra name;
  the pseudocode pack hash is verified. The authority-gated gap families remain promotion/specification
  blockers and are not represented as implemented pseudocode declarations.
- The complete serial all-features Cargo test run passed every binary before the final architecture
  contract exposed two stale workspace assertions. Those assertions and the audited SBI TensorBoard
  side effect were repaired with failing regressions first; both exact affected suites now pass.
- Final warning-denied workspace/all-target/all-feature Clippy, no-default workspace compilation,
  all-package docs, formatting, and diff whitespace pass. Canonical monolithic nextest remains
  unavailable because macOS loader verification stalls its concurrent `--list` discovery; the exact
  attempts and non-equivalent serial evidence are recorded in `VALIDATION_LEDGER.md`.
- No pseudocode implementation blocker remains. Real patient, external, prospective, scale, and
  authority-gated evidence gaps continue to cap claims exactly as recorded. No commit, stage, push,
  deployment, history rewrite, or worktree was created.

## PLAT-DUR-01 durable replay closure checkpoint 52 — 2026-08-26

- Exact cell-table and window source-byte references now join the canonical pattern/window/config
  inputs without copying source data. A file changing during preparation is rejected; scientifically
  equivalent but byte-distinct source input is a durable miss. Native Git provenance now treats an
  untracked source file as dirty.
- Focused evidence passes 13/13 durable CLI cases, 6/6 durable recovery/bound unit cases, 4/4
  ordinary classical CLI cases, and 3/3 ordinary classical workflow cases. The matrix covers exact
  miss/hit and one-object behavior, source/config/resource identity changes, strict head/ledger/
  pending codecs, duplicate cache keys, object corruption/absence, root/control/store path types,
  and all four post-publication recovery points.
- Major-checkpoint formatting, warning-denied workspace/all-target/all-feature Clippy, no-default
  workspace compilation, workspace doc tests, and strict workspace docs pass. The all-features
  serial workspace test command compiled/linked every binary and passed the 294-test root suite
  (21 intentional ignores), API contract 6/6, and the next two integration binaries before manual
  interruption: macOS continued imposing roughly tens of seconds of loader verification per one of
  hundreds of fresh binaries. This partial execution is not a green full-workspace test claim; the
  complete focused affected suites are green and checkpoint 51 retains the last complete serial
  workspace evidence.
- The bounded `PLAT-DUR-01` task is complete. Broader `PLAT-01`/`WF-01` remain active; `BACK-01`
  remains active and `WS-13` is promoted to active. The required local commit was not created
  because the user's explicit instruction forbids committing, staging, pushing, or rewriting.

## Durable external-backend checkpoint 53 — 2026-08-26

- Added `marklab project normal-mean` and `marklab project fused-gromov-wasserstein`. Each exact
  typed workflow now uses `ExecuteAlgorithm`, the existing scheduler, canonical durable project
  head/ledger/recovery transaction, and the existing content-addressed store. Replay decodes and
  revalidates the typed worker result before reconstructing the unchanged one-shot scientific JSON.
- The two immediate callers justify one closed private static descriptor, not a plugin registry. It
  binds backend ID/version, Python 3.12, exact lock and worker digests, recorded SPDX license,
  typed input/output schema identities, deterministic controls, bounded cleared-environment process
  policy, and exact request bytes into the scheduler/durable identity.
- Cross-process PyMC and POT tests each execute one real miss, then succeed as a hit in a second
  process with external-backend startup explicitly disabled. Changed PyMC seed, POT alpha, and
  byte-distinct equivalent input are misses and therefore fail under the same disabled-execution
  control; neither failed miss appends a ledger record. The existing conjugate Normal and reversed
  two-point FGW oracles pass, and exact backend/environment/worker identities match repository bytes.
- Scoped checkpoint evidence passes 41/41 `marklab-bayes` tests, the original PyMC 2/2 and POT 1/1
  CLI suites, durable PyMC 1/1, durable POT 1/1, durable classical 13/13, the focused static identity
  unit test, warning-denied `marklab-bayes` and root-bin Clippy, package/root no-default checks,
  affected formatting, and diff whitespace. No workspace-wide/Nextest/feature-matrix/specialized
  gate was run because this is a two-workflow ordinary checkpoint and checkpoint 51/52 already
  record the Mac loader limitation and latest broad evidence.
- `BACK-01`, `WS-13`, `PLAT-01`, `WF-01`, and `WS-12` remain active: two static durable external
  nodes are not a general registry or dependency-bearing DAG. No dependency, result-format 0.3
  change, remote/container executor, arbitrary task runner, or backend discovery was added. The
  user authorized one local cohesive-checkpoint commit before the next workstream; no push,
  deployment, publication, history rewrite, or worktree was created.

## Dependency, geometry, marks, and design checkpoint 54 — 2026-08-26

- `marklab project marked-prepost` is the first user-facing dependency-bearing durable workflow.
  Two bounded typed result-import nodes feed `MarkedPrePostNode`; the scheduler admits each edge
  only from an exact registered producing specification. A second process restores three hits,
  emits byte-identical result-format 0.3 output, and leaves the three-record durable ledger
  unchanged. Missing, legacy, stale, and genuinely ambiguous outputs fail before execution, while
  byte-identical outputs from two distinct dependencies remain valid edges.
- `ObservationWindow2D` now owns signed boundary distance with positive permitted interior, exact
  zero on exterior/hole boundaries, and negative exterior/hole interior. Classical geometry uses
  that owner for both point admission and retained border distance. A bounded row-aligned
  `MarkTable` now feeds the existing declared marked-analysis node with binary, probability, and
  positive finite nucleus-area columns plus status, modality, unit, provenance, and missingness.
- Patient permutation now compiles its exact blocks, seed, alternative, and replicate controls into
  one private `PatientPermutationDesign`; partial block declaration is rejected instead of silently
  treating undeclared patients as another exchangeability block. These are current-caller slices,
  not completion of general FND-02/FND-04/FND-06 contracts.
- FND-05 production contracts were re-evaluated without mutation. The bounded CellViT CSV/NPY
  importer, exact source-row link, contiguous tables, physical round trips, graph receipts, and
  synthetic consumers already exist. Stable promotion remains data-dependent on unavailable exact
  real-source checkpoint/layer/preprocessing and CellId correspondence; no synthetic substitute or
  provenance claim was fabricated.
- Focused behavior, CLI replay, package-library, warning-denied Clippy, no-default, strict public
  docs, formatting, and whitespace evidence is recorded in the validation ledger. One broader
  cohort-package attempt passed all 20 unit tests and six reference binaries, then was interrupted
  during the documented macOS per-binary verification delay; it is not claimed as a package pass
  and was not retried. No broad workspace/Nextest/feature-matrix gate, commit, stage, push,
  deployment, publication, history rewrite, or worktree was created for checkpoint 54.

## Typed spatial autocorrelation checkpoint 55 — 2026-08-26

- Added a public global Moran workflow over the existing dense positive nucleus-area mark. It binds
  the exact typed `MarkTable`, a coordinate-frame-bound `ObservationWindow2D`, one fixed-radius
  `SpatialIndex2D` edge plan, explicit binary-symmetric or row-standardized weights, and bounded
  deterministic random-labeling inference. The four-point chain matches the hand values `I=0.4`
  and row-standardized `I=0.54`; alternate/unbound frames, invalid categorical codes/units,
  isolated points, and permutation-work overflow fail explicitly.
- `MarkTable` now has one immediate-caller categorical specialization for exact
  `histologic_compartment` codes, ordered level labels, measurement status, histology modality,
  categorical unit, missingness, and provenance. Moran conditioning consumes and reports this typed
  column rather than reading the compatibility stratum map directly; the adapter still verifies
  exact row equality with that preserved input format.
- Promoted `marklab-cohort::InferenceDesign` as the smallest contract shared by patient-label and
  cell-mark random labeling. It owns analysis level, null family, whole-unit permutation kind,
  complete exact blocks, replicate count, seed namespace, alternative, and single-endpoint
  multiplicity, and rejects partial blocks, singleton-only designs, invalid partitions, and
  out-of-range replicates. Both patient reference oracles and the Moran workflow pass unchanged.
- This advances FND-02/FND-04/FND-06, SIG-01/SIG-01A, WS-22/WS-23/WS-31. It does not implement
  polygon compartment partitions/interfaces, general categorical/ordinal/simplex/vector marks,
  local Moran maps, Geary/variogram integration, covariate residualization, broader multiplicity,
  patient-level spatial comparison, external PySAL/R agreement, or real-data promotion. FND-05
  remains data-blocked on the exact source authority recorded at checkpoint 54.

## Durable typed Moran graph checkpoint 56 — 2026-08-26

- Added typed global-Moran source nodes and a typed descriptive pre/post node. Two exact typed
  MarkTable/window/design analyses now form a real three-node dependency graph whose downstream
  result retains both source results and the finite post-minus-pre Moran-I difference without
  claiming paired inference.
- The source node binds exact pattern bytes, typed MarkTable identity and provenance artifacts,
  framed observation-window identity, mark/radius/weight policy, deterministic design/seed, resource
  limits, implementation identity, and a strict private version-one codec. The dependent node
  accepts only exact scheduler-produced Moran artifacts with compatible estimands and controls.
- Added the narrow store-aware form of `execute_algorithm`; it reuses the existing scheduler's
  semantic-input verification and the unchanged durable object, ledger, head, pending-intent,
  recovery, cache-key, and output transaction owners.
- A fresh second fixture and reopened durable project return three hits with an unchanged
  three-record ledger. Changing only the seed produces three misses and three additional records,
  proving exact configuration identity. Focused Moran, dependency-replay, workflow-library,
  warning-denied Clippy, no-default, strict-doc, formatting, and whitespace checks pass.
- WF-01/WS-12 and FND-06 advance but remain active. General graph construction, resource planning,
  parallel scheduling, whole-graph resume, inferential longitudinal comparison, broader
  multiplicity, and patient-independent spatial comparison remain outside this checkpoint.
