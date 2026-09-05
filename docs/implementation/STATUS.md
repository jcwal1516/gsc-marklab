# Implementation status

Last updated: 2026-09-05

## Current outcome — RUST-MIGRATION-01

The user-authorized migration is now the active workstream (DEC-0415). All 154 production worker,
adapter and client Python sources at baseline `f784302` remain in scope. The generated
`PYTHON_MIGRATION_INVENTORY.csv` freezes source identities and literal caller/contract matches;
proposed ownership and unmatched/dynamic callers still require individual review. It is not a
completed capability-by-capability migration or a reachability proof.

The first native candidate implements the complete grouped-conformal CSV workflow through a
library application, Bayesian scientific owner, bounded numerical optimizer and killable native
child. It preserves training/calibration/test separation, corrected rank, inclusive sets, coverage,
resource bounds, the claim ceiling and legacy Python readers. Native envelope version 2 truthfully
identifies Rust execution; result-format 0.3 is unchanged. Python sources and locks remain frozen.
The grouped-conformal milestone is complete. The corrected native candidate passes nine domain
integration tests, three internal gradient/hash/overflow oracles and six CLI tests. The new native
CI command passes all nineteen selected numerical/domain/CLI cases with Python execution disabled.
On three independently converged Python fixtures, ten paired repetitions show 7.29–8.26x cold and
4.45–7.59x warm median speedups. All eleven other reference cases retain their SciPy precision-loss
failures; fixed-native-parameter Python gradient/downstream checks pass but are not independent fits
or speedup comparisons. See `../native-migration-measurements.md` and its raw audit record.

At the preceding grouped-conformal major checkpoint, the default-concurrency workspace run stopped at one unchanged hierarchy's 180-second backend
timeout (457 passed; 1266 not run). The unchanged test passed in isolation in 117.78 s. The full
CI-concurrency rerun passed 1724/1724 in 2849.705 s, with 12 slow tests and 28 existing skips.
That suite used the snapshot before the final contained standardization-order correction; its
new failing regression was fixed, and current domain/CLI tests plus all-feature workspace Clippy
pass afterward. A fresh unfiltered 1725-test run is not claimed. Formatting, earlier CLI-only
Clippy/no-default/CSV-only/strict-doc/doc-test gates and scoped offline package checks are recorded
in the task contract. `actionlint .github/workflows/ci.yml` could not run because actionlint is not
installed; Ruby Psych syntax checks, the CI contract and the exact native test command pass locally.
Hosted CI, cross-platform execution, final native release and the remaining Python migrations remain open.

The native patient-OOF calibration milestone (DEC-0416) now adds the complete CSV/application/child
workflow, all three regressions, held-out metrics and truthful native provenance. Eight frozen Python
cases pass differential checks; analytical constant scores, leakage, ordering, finite saturation,
malformed/resource limits, exact-score transport and legacy reading are covered. Ten alternating
paired repetitions show 2.66–14.64x warmed speedups on all eight cases and 7.39–8.24x cold speedups
on six successful complete comparisons. The unchanged legacy CLI rejects the two original large
decimal-score cases; they retain warmed parity/timings but no cold speedup claim. Raw samples,
RSS, profiles, identities and harness corrections are in `audits/prediction_calibration_measurements.json`.
Scoped Bayesian/root Clippy and the expanded native CI selection pass; final focused calibration
checks are recorded in the validation ledger. The earlier full-workspace gate is not represented
as a fresh calibration checkpoint. Python workers/locks, result-format 0.3 and all parent claims remain.

Late fusion is now native as well (DEC-0417), including meta-only fitting, disjoint Platt calibration,
observed missingness scenarios and every fixed-model modality ablation. It shares only the exact
logistic likelihood/optimizer with conformal; preprocessing and standalone calibration remain separate.
Three independently converged references pass ten-pair timing gates: 6.77–11.34x warm and
7.98–8.36x cold speedups. Three other SciPy failures remain recorded; fixed-native-parameter Python
objective/gradient and downstream checks pass but do not establish independent optimizer parity.
The conformal shared-solver recheck passes at 7.06–8.36x cold and 2.38–6.31x warm, with the latter
now measuring the larger actual CSV/control application boundary. See `audits/late_fusion_measurements.json`.

The three-workflow stabilization checkpoint passes all 1744 workspace tests at the established
`--test-threads 2` concurrency (2763.465 s; 12 slow, 28 existing skips). The current native CI selection
passes 38/38 with Python execution disabled. Workspace all-feature Clippy, no-default checking,
strict docs, all seventeen doc-test targets (zero runnable cases), formatting and the added conformal
application-example lint pass. The default-concurrency command that timed out at the prior checkpoint
was not rerun; this result explicitly uses two-test concurrency. No full phase/release matrix,
new packaging/audit/fuzz gate, hosted CI or final Python-free release is claimed.

Next: entropic partial transport, then patient-grouped predictive stacking and the remaining
deterministic/adaptor workflows. The remaining 151 frozen Python inventory rows still require
individual admission. Real multiplex study admission follows the migration; its biological design,
assay metadata and independent calibration gaps remain.
The previous architecture checkpoint remains recorded below with its original limitations.

## Previous outcome — ARCH-INTEGRATION-01

User-authorized architecture integration has delivered the bounded workflow below. Earlier local milestones:

- `3f56e3b`: one composed command tree owns help, validation and execution routing. All original
  method handlers and mathematical owners remain; the Bayesian parser is grouped to avoid its
  large debug stack frame.
- `f67f36e`: shared runtime asset/interpreter/cache resolution, installation doctor, release worker
  assets, and an actual relocated-binary fit with byte-identical backend-free durable replay.
- `3fc3dae`: provisioned required backend CI and standalone Python tests. Those tests exposed a
  missing production Shapely dependency; the pinned lock adds only Shapely 2.1.2.

The multiplex outcome is implemented: named nullable assay columns in the shared MarkTable,
observed-cell Moran/Geary graphs, equal-slide patient Max-T, one library application, durable
slide/collection/cohort execution and atomic claim-bounded reports. The CLI and thin Python client
use the same service. Real AnnData 0.12.4/H5AD interchange preserves row identity, sparse selected
values, annotations, nulls and explicit physical coordinates. Existing legacy public mark enums,
Pattern projection and result-format 0.3 remain unchanged (DEC-0412–0414; IC-0201).

Combined stabilization is complete with the bounded correction described below. Final focused
numerical tests pass 17/17; scalar compatibility passes 11/11; CLI discovery/process replay passes 6/6; corrected real backend regressions pass 10/10;
Python suites pass 76 scientific + 1 worker + 3 real-client tests. Final formatting, all-feature and
CLI-only warning-denied Clippy, no-default workspace compilation, strict documentation and all
seventeen workspace doc-test targets pass (zero doc cases). The complete all-feature nextest run
executed all 1,701 selected tests in 2,816.825 s: 1,700 passed (12 slow), one CI-contract assertion
failed, and 28 existing tests were skipped. Its blanket `python/tests` ban incorrectly matched the
new `clients/python/tests` path. The contract now requires both locked environments and installation
before their required test commands while retaining the obsolete root-package prohibition. All
seven focused workflow tests pass under both Cargo test and nextest after that correction;
warning-denied test Clippy also passes. No production
code or CI configuration changed, and the expensive unfiltered suite was not repeated; a fully green
single unfiltered run is not claimed.

The suite exposed and corrected the lost explicit anisotropic-GP command name, PyMC cumulative
rather than per-transition divergence extraction, and an ineffective Python hash seed under `-I`.
Both runners now share a cleared-environment `-P -s -B` policy, preserving import exclusion and
existing diagnostic thresholds. The relocated-binary fixture explicitly selects its source assets.
The earlier full runs remain recorded failures/interruption in the task contract, not green gates.

The optimized Unix archive smoke runs doctor and the native example after extraction to a path
with spaces, with no asset override. A 31-second seeded fuzz run completes 174,252 inputs without a
reported failure. Complete 72/60,000-cell synthetic workloads pass twelve cold/replay runs with
exact scientific output and ledger/count oracles; the larger workload's median is 0.86 s cold and
0.17 s replay, peak RSS below 50 MiB. Concurrent checks and warm filesystem caches limit timing
interpretation; see `docs/multiplex-study-measurements.md` for all samples and hashes.

All 233 tracker rows remain, and the previous 2,091-line active roadmap is preserved verbatim in
ROADMAP_HISTORY. The bounded roadmap now promotes real multiplex input admission, independent
study calibration and measurement of that admitted workload. Parent scopes remain active. The
capability and multiplex guides describe available behavior and claim limits.

Remaining: clean hosted CI, Windows/Linux backend execution, WSI-enabled release-matrix validation,
real pathology panel admission, independent biological calibration and whole-slide inference
capacity. Broader binary-service migration, universal result descriptors, arbitrary workflow
construction, SpatialData/OME-NGFF and R/zero-copy clients remain caller-dependent work. The latest
real joint location–embedding pilot remains diagnostic-only; this checkpoint does not promote it.

## Historical implementation and checkpoint records

The records below retain their original dates, identities, evidence and limitations. Their older
"current" labels describe their own checkpoint, not the active outcome above.

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

## Typed global Geary checkpoint 57 — 2026-08-26

- Added global Geary's C as a distinct typed result and public workflow over the exact framed
  MarkTable input, radius edges, weight normalization, deterministic random-labeling design,
  compartment blocks, and resource ceilings already consumed by global Moran's I.
- The four-point chain matches independent hand calculations for both admitted policies:
  binary-symmetric `C=0.38` and row-standardized `C=0.2925`. The corresponding Moran and Geary
  runs retain the same exact weights digest, while their formulas, null expectations, alternatives,
  and result types remain distinct.
- SIG-01B is complete for this bounded native method. SIG-01 remains active for scalar variograms,
  broader weight plans, external agreement, covariate residualization, scale, and real-data
  validation; local/bivariate maps remain gated on multiplicity and cohort-valid use.

## Observed scalar semivariogram checkpoint 58 — 2026-08-26

- Added a bounded public observed scalar semivariogram over the exact typed continuous mark and
  coordinate-frame-bound window. Callers declare finite contiguous micrometre lag bins; the result
  retains exact typed-input/provenance identity, a canonical row/coordinate/window/bin plan digest,
  unordered-pair visits and counts, optional empty-bin values, and explicit correction/inference
  status.
- The four-point line matches hand semivariances `19/3`, `24.5`, and `32` for pair counts `3,2,1`.
  The all-pair ceiling is checked before traversal and rejects the six-pair fixture at a five-visit
  limit. Duplicate physical points, frame/window drift, row mismatch, invalid bins, non-finite
  accumulation, and missing typed marks fail explicitly.
- SIG-01F/FND-03 advance but remain active: this checkpoint truthfully reports
  `none_fixed_observed_locations` and `observed_only_no_null`. Permutation envelopes, justified pair
  edge correction, directional variants, external agreement, scale evidence, and real validation
  remain before complete status.

## Three-workflow stabilization checkpoint 59 — 2026-08-26

- Stabilized the durable typed Moran graph, global Geary C, and observed scalar-semivariogram
  milestones as one three-workflow boundary. Workspace-wide all-target/all-feature warning-denied
  Clippy, workspace no-default compilation, all-feature workspace doc tests, strict all-feature
  workspace docs, formatting, and whitespace checks pass.
- The full workspace integration suite and Nextest were not rerun: checkpoints 51 and 52 document
  the complete prior serial evidence and the reproducible macOS binary-verification stall, and the
  active instruction explicitly forbids retrying that loop. Focused changed data flows are green at
  checkpoints 56–58.

## Scalar-semivariogram ERL inference checkpoint 60 — 2026-08-26

- Added deterministic whole-value random labeling for the existing scalar semivariogram, either
  globally or within the exact typed `histologic_compartment` codes. The implementation reuses
  `marklab-cohort::InferenceDesign` and the existing `marklab-numerics` two-sided
  extreme-rank-length envelope rather than adding another permutation or multiplicity engine.
- The result retains the observed curve unchanged, simultaneous bounds only for nonempty lag bins,
  one inclusive-plus-one global p-value, observed/critical ERL depths, alpha, seed, completed
  permutations, stratum and conditioning identity/status, and the explicit curve-family policy.
  The four-point fixture replays exactly and a 186-evaluation request fails against a ceiling of 185
  before generating a null curve.
- SIG-01F/FND-06 advance but remain active for directional estimands, justified pair edge
  correction, external agreement, representative scale, patient-level comparison, and real-data
  validation. No pointwise tests, hidden scale selection, new null registry, or result-format 0.3
  change was added.

## Real CellViT result checkpoint 61 — 2026-08-26

- Delivered `RESULTS-CELLVIT-2DAY-01` against the user-authorized Mac mini CPTAC-COAD corpus. The
  bounded admission rehashed all three declared outputs for 366 completed slides and verified
  1,542,389 finite row-aligned 1,280-dimensional CellViT cell embeddings, physical coordinates,
  five-class annotations/probabilities, 4,392 selected-patch links, 178 patient identities, the
  frozen source/model/environment/protocol identities, 624,284 projected tumor-cell rows, and the
  linked spatial and clinical manifests. The inference runner was inactive and its prior complete
  tree verification had zero failed or partial slides.
- Ran real coordinate-only conditional-CSR K/L, pooled probabilistic scalar-mark spectrum, raw
  1,280-dimensional vector semivariogram, split-safe 16-component projected variograms, exact
  cell-to-patch-to-patient multiscale aggregation, nested patient-held-out cell/patch
  complementarity, patient-unit four-endpoint Max-T, and pinned PyMC normal-mean workflows. The
  coordinate and scalar endpoints returned global permutation p-values of `0.01`; raw vector
  semivariance increased across the four declared 0–200 micrometre bins; all projected train,
  validation, and test component/bin families were populated.
- The real 96-patient nested analysis did not support a cell-feature increment over technical,
  age, compartment, and acquisition covariates (`p=0.14`) or an incremental patch contribution
  after cell features (`p=0.84`). The 98-patient MSI/MSS Max-T family found no adjusted endpoint below
  `0.238`. These are exploratory workflow outputs, not clinical, calibration, causal, or
  performance evidence. Cell-aggregated patch embeddings are admitted; an independent raw
  patch-vector tensor is the only unavailable lane and carries that exact blocker.
- A real polygon calculation exposed strict redundant-float equality in the classical private
  codec. One-ULP regression coverage now passes through a sixteen-epsilon relative recomputation
  tolerance while exact-zero, count, denominator, status, and tamper checks remain strict. The
  durable coordinate project reopens as a hit with one ledger record and identical analysis bytes.
  PyMC 6.3.0 completed 2,000 draws with finite diagnostics, zero divergences, `R-hat=1.0074`, and a
  posterior patient-organization mean of `0.07398` (interval `0.06979`–`0.07830`); a second process
  with external backend execution disabled returned a byte-identical hit and left one ledger row.
- The closed 54-file bundle was sealed and verified locally and at
  `/Volumes/1TB/marklab/runs/results-cellvit-2day-01`; a one-byte copied-result mutation was
  rejected. Focused tests, root-package CLI warning-denied Clippy, root no-default compilation,
  affected formatting, and Python syntax checks pass. The checkpoint does not claim an independent
  patch model, exhaustive whole-slide inference, publication readiness, or completion of the
  broader embedding/foundation master-plan families.

## Durable real Bayesian checkpoint 62 — 2026-08-26

- Added `marklab project hierarchical-normal` and `marklab project gridded-lgcp` as typed PyMC
  nodes over the existing durable scheduler, object store, ledger, head, recovery, cache key, and
  output transaction. Both first execute as misses and then replay byte-identical typed results in
  separate processes with external backend execution disabled and one ledger record. Exact raw
  input bytes, PyMC 6.3.0, Python 3.12, lock and worker digests, model/configuration, seeds,
  resources, typed schemas, and native runtime identity remain in the durable boundary.
- The hierarchical workflow fit `284` real spatial cosine-excess observations nested in `121`
  patients after excluding only the `49` single-ROI patients required by the model's declared
  replication contract. The complete fit has zero divergences, `R-hat=1.0046`, bulk ESS `1505`,
  tail ESS `2051`, population mean `0.07715`, between-patient SD `0.02305`, finite prior/posterior
  checks, and posterior-predictive agreement on the declared global and patient-dispersion
  summaries.
- A narrow CellViT adapter rehashed the admitted representative-slide cell export and selected all
  `500` cells assigned to exact patch `(7,16)`, preserving its physical
  `256.1024 x 256.1024` micrometre rectangle. The durable 4x4 gridded LGCP returned a complete
  latent-field/point-process fit with zero divergences, `R-hat=1.0032`, bulk ESS `896`, tail ESS
  `950`, and posterior-predictive total `499.31` against `500` observed. The x-grid covariate is a
  declared spatial trend, not a biological effect, and the result remains exploratory.
- Added the first direct pinned NumPyro 0.21.0/JAX 0.11.1 backend caller for the exact same typed
  Gaussian hierarchy. On the real ROI input, PyMC and NumPyro population means differ by
  `0.000103` and between-patient SDs by `0.0000275`; intervals overlap and both differences pass
  the declared four-MCSE/minimum-`0.005` rule. Both engines have zero divergences and passing
  normalized rank-R-hat, bulk/tail ESS, energy, finiteness, constraint, and predictive checks.
- This advances BAY-01 through its first two-backend hierarchical promotion model and advances
  BAY-02/BAY-03/BAY-04/BAY-PP plus WS-40/41/43/44. The broader Bayesian phase is not complete:
  hierarchical SBC/prior sensitivity and field/point-process cross-backend agreement, SBC, and
  spatial posterior-predictive checks remain the next concrete blockers; CmdStan and actual GPU
  execution evidence also remain absent.

## Hierarchical calibration and five-workflow stabilization checkpoint 63 — 2026-08-26

- Added an explicit five-fit hierarchy prior-sensitivity workflow. It holds the real 121-patient,
  284-ROI input, known observation scale, seed, sampler, and likelihood fixed while changing the
  global-mean and between-patient prior scales one at a time to `0.5x` and `2x`. All five real fits
  pass the unchanged diagnostic policy with zero divergences. No posterior shift reaches the
  declared material threshold of `0.5` baseline posterior SD; the maximum is `0.128`, so the result
  is stable only within this declared grid and does not claim universal prior robustness.
- Added bounded NumPyro simulation-based calibration for the exact Gaussian patient hierarchy. A
  final 40-replicate, 32-patient, two-observation design used the real workflow's priors and known
  sigma with two chains, 4,000 warmup and 8,000 retained draws per chain. All 40 fits pass: zero
  divergences, maximum `R-hat=1.00435`, minimum bulk/tail ESS `570/1019`, rank-uniformity p-values
  `0.163` and `0.312`, and 90% coverage `0.875` for both population mean and heterogeneity.
- Two earlier bounded SBC schedules are preserved as diagnostic-only outputs. The 1,000-draw run
  failed 21/40 per-fit diagnostic gates; a 4,000-draw run reduced that to 2/40. No ESS/R-hat,
  rank-uniformity, coverage, or failure-disposition rule was relaxed to obtain the final pass.
- The five related workflows from checkpoints 62-63 passed one stabilization cycle: workspace
  all-target/all-feature warning-denied Clippy, workspace no-default compilation, all-feature
  workspace doc tests, strict all-feature workspace docs, formatting, and whitespace. Focused
  hierarchy/LGCP durability, agreement, sensitivity, SBC, and adapter tests also pass. The
  documented full-integration/Nextest macOS loader loop was not rerun.
- The Gaussian hierarchy now satisfies its current cross-backend, posterior-predictive,
  sensitivity, and SBC gates. Phase 4 remains active for the real gridded field/LGCP caller's
  independent-backend agreement, SBC, prior/kernel sensitivity, and spatial posterior-predictive
  summaries, plus the broader likelihood/model families and absent CmdStan/GPU evidence.

## Gridded-LGCP calibration stabilization checkpoint 64 — 2026-08-27

- Added an independent NumPyro 0.21.0/JAX 0.11.1 fit for the exact PyMC 6.3.0 fixed-grid,
  fixed-Matérn LGCP request. The real 16-cell CellViT comparison passes global, latent-field, and
  expected-count gates with zero divergences: intercept/coefficient differences are `0.01999` and
  `0.02105`, latent-effect RMS difference is `0.02695`, and expected-count RMS difference is
  `0.11365`.
- Added bounded exact-model SBC. The first real-design 1,000-draw schedule retained three ESS
  failures and remains diagnostic-only. The final 20-replicate schedule used two chains, 2,000
  warmup, and 4,000 draws per chain; all 20 pass without relaxed gates, with zero divergences/depth
  hits, maximum `R-hat=1.00253`, minimum bulk/tail ESS `1229/1618`, and passing intercept,
  coefficient, and prespecified latent-cell rank/90% coverage checks.
- Added a 32-pattern spatial posterior-predictive result over the existing exact-cell simulator.
  The real observed/replicated cell-count variances are `110.19/141.59` with tail `0.75`; adjacent
  mean absolute contrasts are `10.375/11.197` across 24 grid-neighbor pairs with tail `0.625`.
- Added the nine-fit 0.5x/2x one-at-a-time intercept-prior, coefficient-prior, Matérn-amplitude,
  and Matérn-length sensitivity grid. An initial length-scale-lower fit missed bulk ESS and is
  preserved as diagnostic-only. The final 2,000-warmup/2,000-draw grid passes all fits with zero
  divergences/depth hits, maximum `R-hat=1.00427`, minimum bulk/tail ESS `644/929`, and maximum
  standardized shift `0.446` below the declared `0.75` threshold.
- Four focused CLI integrations and the original PyMC fit oracle pass; `marklab-bayes` passes 41/41
  package tests. Workspace warning-denied all-target/all-feature Clippy passes in 13m07s;
  workspace no-default, all-feature doc tests, strict all-feature docs, formatting, and whitespace
  checks pass. The documented full-integration/Nextest macOS loader loop was not rerun.
- This closes the current exact Gaussian-hierarchy and fixed-grid-LGCP calibration gate, not the
  broader Bayesian phase. CmdStan, actual GPU evidence, non-Gaussian/repeated/crossed/varying-slope
  hierarchies, inferred field hyperparameters, arbitrary windows, and broader fitted marked or
  replicated point-process families remain.

## Robust non-Gaussian hierarchy stabilization checkpoint 65 — 2026-08-27

- Added a genuine typed patient beta-binomial likelihood with population mean/concentration,
  patient probabilities, overdispersion, partial pooling, posterior-predictive totals and
  dispersion, exact PyMC identity, and bounded diagnostics. Its synthetic oracle is complete; no
  real successes/trials result is claimed because the current admitted Bayesian input bundle does
  not yet contain a provenance-complete numerator/denominator table.
- Added a finite-variance Student-t patient hierarchy for the real `121`-patient/`284`-ROI cosine-
  excess input. The PyMC fit is complete with population mean `0.07699`, between-patient SD
  `0.02313`, observation SD `0.01963`, degrees of freedom `19.27`, `R-hat=1.00227`, zero
  divergences/depth hits, and a declared robust-residual posterior-predictive tail of `0.752625`.
  The exact typed workflow now runs durably as miss then byte-identical backend-disabled hit with
  one unchanged ledger row.
- Independent dense-mass NumPyro and PyMC fits agree on the real input: population mean,
  between-patient SD, observation SD, and degrees-of-freedom differences are `0.0000738`,
  `0.0001115`, `0.0000186`, and `0.08366`; patient means have RMS difference `0.0001768`; both
  backends report zero divergences/depth hits. All nine 0.5x/2x one-at-a-time prior/tail scenarios
  converge, but the lower degrees-of-freedom-rate scenario is materially sensitive (`1.0558`
  baseline posterior SD), so robustness is not claimed outside the declared baseline/grid.
- Added exact prior-generative Student-t SBC with dense-mass NumPyro, complete replicate
  disposition, rank and 90% coverage for all four population parameters, depth-12 execution, and
  unchanged convergence gates. The real `121`-patient/`284`-observation shape passes 20/20 at two
  chains, 2,000 warmup and 4,000 draws: maximum `R-hat=1.00277`, minimum bulk/tail ESS
  `1065/2121`, minimum E-BFMI `0.441`, zero divergences/depth hits, rank p-values
  `0.437`–`0.911`, and coverage `0.80`–`0.95`. A four-chain schedule exceeded its 1,200-second
  wall-clock bound and published no artifact; no diagnostic or calibration gate was relaxed.
- The result SHA-256 `b2b192a45c19264d9eb44c8c6341725078cba59a46c14e4f925739235db239c4`
  matches locally and on the authorized Mac mini 1 TB volume. Five affected CLI/project
  integrations, the 44-test Bayesian library, workspace warning-denied all-target/all-feature
  Clippy, workspace no-default compilation, all-feature doc tests, strict docs, formatting, and
  whitespace checks pass. The documented full-integration/Nextest loader loop was not rerun.
- This closes the current robust Student-t promotion gate, not BAY-03/BAY-HIER-A. A real
  provenance-complete count numerator/denominator caller is next; repeated/crossed/varying-slope
  structures still require an admitted design that actually identifies those effects, and
  CmdStan/actual GPU evidence remain absent.

## Real CellViT count-hierarchy stabilization checkpoint 66 — 2026-08-27

- Extended the existing full-corpus CellViT adapter to emit exact patient counts after rehashing and
  revalidating all 366 slides and 1,542,389 row-aligned annotations. Success is the unique source
  class ID `1`, `Neoplastic`; trials are every admitted hard-classified cell across all admitted
  slides for each patient. The sorted table contains 178 patients, 625,276 successes, and 1,542,389
  trials; all 11 prior prepared inputs reproduced byte-for-byte. The contract explicitly states
  that classifier outputs and spatially correlated cells are not independent biological Bernoulli
  trials. An initial remote invocation lacked the recorded CellViT source root on `PYTHONPATH`,
  processed no data, and is preserved as a dependency failure; the corrected pinned run passed.
- The real PyMC beta-binomial hierarchy is complete: population Neoplastic-class probability
  `0.37574` (95% interval `0.34876`–`0.40354`), concentration `5.7277`, implied overdispersion
  `0.14968`, `R-hat=1.00170`, bulk/tail ESS `9369/4738`, and zero divergences/depth hits. Posterior-
  predictive total successes are `625269.98` versus `625276` observed, and replicated versus
  observed patient-proportion SD is `0.18341/0.18360`.
- The workflow now runs durably as miss then byte-identical backend-disabled hit with one ledger
  row. Independent NumPyro/PyMC fits agree: population probability differs by `0.000166`,
  concentration by `0.01142`, patient probabilities have RMS/max differences
  `0.0000772/0.000431`, all intervals overlap, and both backends have zero divergences/depth hits.
  The complete seven-fit 0.5x/2x prior grid has maximum standardized shift `0.0886`, below the
  declared `0.75` material threshold.
- Exact prior-generative SBC first exposed a latent-probability funnel: 19 fits passed, but replicate
  13 hit depth 12 for all 6,000 draws with `R-hat=2.12`; that diagnostic-only artifact is retained.
  The mathematically equivalent collapsed beta-binomial likelihood plus exact conditional patient-
  probability draws passes 20/20 without changing priors or gates: maximum `R-hat=1.00143`, minimum
  bulk/tail ESS `2758/2436`, minimum E-BFMI `0.933`, zero divergences/depth hits, rank p-values
  `0.637`–`0.911`, and 90% coverage `0.85/0.85/0.90`.
- All admitted inputs/results and the durable project are mirrored on the authorized Mac mini 1 TB
  volume with matching SHA-256 identities. Focused adapter, one-shot, durable, agreement,
  sensitivity, SBC, and 44-test Bayesian-library checks pass. Workspace warning-denied
  all-target/all-feature Clippy, no-default compilation, all-feature doc tests, strict docs,
  formatting, and whitespace checks pass. The documented full-integration/Nextest loader loop was
  not rerun.
- This closes the current unadjusted real beta-binomial promotion gate, not BAY-03/BAY-HIER-A. The
  next concrete caller is patient-unit molecular-group beta-binomial regression over the exact
  admitted count/label intersection; ordinal, hurdle, longitudinal/crossed, CmdStan, and actual GPU
  evidence remain blocked or absent.

## Patient molecular-group hierarchy stabilization checkpoint 67 — 2026-08-27

- Extended the full-corpus CellViT adapter with an exact patient-ID join between the already
  admitted Neoplastic successes/all-classified-cell trials and the pinned MSI/MSS label manifest.
  The resulting SHA-256 `e9a875daf3456895d31ecb8175de9e44637ebf3633fb6e8fb4ed16c1b2b6d6a1`
  table contains 105 biological patient units: 81 MSS patients with 300,386/693,968 successes/trials
  and 24 MSI patients with 71,886/192,783. Seventy-three admitted count patients lacking either
  label are excluded explicitly. The full 366-slide/1,542,389-cell admission rerun reproduced every
  prior prepared input byte-for-byte; classifier outputs and spatially correlated cells remain
  explicitly non-independent biological trials.
- Added a typed collapsed patient beta-binomial group regression with reference/comparison log-odds,
  group probabilities, probability difference, odds ratio, concentration/overdispersion, exact
  conditional patient probabilities, group-aware posterior predictive checks, deterministic seed,
  exact backend/input identity, and hard patient/trial/iteration/output/time bounds. The real PyMC
  fit is complete: MSS probability `0.39731`, MSI probability `0.30371`, MSI-minus-MSS difference
  `-0.09360` (95% interval `-0.16749` to `-0.01535`), odds ratio `0.66941` (95% interval
  `0.46433`–`0.93598`), concentration `6.1842`, `R-hat=1.00047`, and zero divergences/depth hits.
  Aggregate-count posterior-predictive upper tails are `0.12725` for MSS and `0.105375` for MSI;
  the result remains exploratory composition evidence, not a clinical or causal claim.
- `marklab project beta-binomial-group-regression` ran the real input as a miss and then a
  byte-identical backend-disabled hit in a separate process with one ledger row. Both outputs have
  SHA-256 `f51dfff193af353173c33a3bb159a1bb40b1bbf9d6911ed31b206f42a046c603`.
  Independent dense-mass NumPyro/PyMC fits agree on every declared population estimand and all 105
  patient probabilities: the probability-difference discrepancy is `0.000897` (`1.314` combined
  MCSE), patient RMS/max discrepancies are `0.000103/0.000385`, all intervals overlap, and both
  backends report zero divergences/depth hits.
- The complete seven-fit 0.5x/2x intercept-, group-effect-, and concentration-prior grid is stable
  within its declared `0.75`-SD threshold; the maximum shift is `0.216` SD under the tighter group-
  effect prior. Exact prior-generative NumPyro SBC over the real 105-patient/886,751-trial shape
  passes 20/20 with no failures, maximum `R-hat=1.00117`, minimum bulk/tail ESS `4994/4014`, minimum
  E-BFMI `0.944`, zero divergences/depth hits, rank p-values `0.091`–`0.911`, and 90% coverage
  `0.80`–`0.95` across intercept, group effect, concentration, probability difference, and one
  exact-conditional patient coordinate.
- All current inputs, durable state, and results are mirrored without deletion under
  `/Volumes/1TB/marklab/runs/results-cellvit-bayesian-v1`. Focused one-shot, adapter, durable,
  agreement, sensitivity, SBC, and 45-test Bayesian-library checks pass. At the five-milestone
  boundary, workspace warning-denied all-target/all-feature Clippy passed in `14m54s`; workspace
  no-default compilation, all-feature doc tests, strict all-feature docs, formatting, Python syntax,
  and whitespace checks pass. The documented full-integration/Nextest macOS loader loop was not
  rerun.
- This closes the first real patient-unit Bayesian group-effect promotion gate, not BAY-03 or
  BAY-HIER-A. The next identified Bayesian hierarchy needs a real immediate caller for more than one
  covariate or repeated/crossed/random-slope structure; ordinal/hurdle, CmdStan, and actual GPU
  evidence remain absent and will not be fabricated.

## Gender-adjusted molecular-group hierarchy stabilization checkpoint 68 — 2026-08-27

- The bounded clinical admission found one smallest complete adjustment caller: the pinned `Gender`
  row joins exactly to all 105 MSI/MSS patients, with design support of 46 MSS Female, 35 MSS Male,
  17 MSI Female, and 7 MSI Male patients. The sorted count/label/gender table has SHA-256
  `8f6e6c433d34793b31ad59255e73194732db448390c8196715c96e247e84e4e0`, and the admission record has
  SHA-256 `3c8fa133737880561dd127e15d7b9996c2969cdcfc2688a4fe3e20ca09951fda`. The full 366-slide,
  1,542,389-cell adapter rerun preserved every previously admitted input byte-for-byte. Age was not
  used because two patients are missing it and the source does not declare its units; no silent
  complete-case deletion or inferred unit was introduced.
- Added the exact additive patient beta-binomial group-plus-gender model with no interaction, shared
  concentration, exact conditional patient probabilities, four design-cell probabilities, and an
  MSI-minus-MSS contrast standardized to the observed gender distribution. The complete PyMC fit
  estimates a group log-odds effect of `-0.41124` (95% interval `-0.76378` to `-0.06739`), gender
  effect `-0.05128` (`-0.35349` to `0.23992`), marginal MSS/MSI probabilities `0.39758/0.30533`,
  marginal difference `-0.09226` (`-0.16633` to `-0.01572`), group odds ratio `0.67336`, and
  concentration `6.1471`. `R-hat=1.00123`, divergences/depth hits are zero, and the aggregate,
  group-difference, and gender-difference PPC tails are `0.06525/0.412/0.533625`. The near-identical
  unadjusted difference was `-0.09360`; this does not establish general confounding control or a
  causal/clinical effect.
- The real one-shot and separate-process durable miss/backend-disabled hit are byte-identical with
  SHA-256 `f88ca744929d3ea492fffb33bdd9fb2083dcc77eb7cabe9abd7ffafd2de90499` and one ledger row. Pinned
  PyMC and dense-mass NumPyro agree on all 15 scalar estimands and all 105 patient probabilities;
  the group/gender effect discrepancies are `0.000422/0.001986`, the marginal-difference discrepancy
  is `0.000159`, patient RMS/max discrepancies are `0.000154/0.001254`, and both fits have zero
  divergences/depth hits. The agreement result SHA is
  `8023c116ba1d2fd787b0e207332f6e226ae3e47b2d29b9afedcf16766298caea`.
- All nine baseline plus 0.5x/2x one-at-a-time prior fits converge and remain below the declared
  `0.75`-SD material threshold; the maximum shift is `0.21837` SD for the tighter group-effect prior.
  The sensitivity result SHA is `be23541b9a36c785a69ace3193493b96684641430798f8cfcf18c56576ddef2c`.
  Exact prior-generative SBC over the real 105-patient/886,751-trial group/gender shape passes 20/20
  with no failures, maximum `R-hat=1.00204`, minimum bulk/tail ESS `6505/4579`, minimum E-BFMI
  `0.916`, zero divergences/depth hits, rank p-values `0.350`–`0.964`, and 90% coverage `0.85`–`0.95`
  across intercept, group effect, gender effect, concentration, the standardized marginal contrast,
  and one exact-conditional patient coordinate. Its SHA is
  `4d148ba728fb3bea6d22f076be3c6c754c373da8b1fc9a86b8171711477cb60c`.
- Inputs, durable state, and results are mirrored without deletion under
  `/Volumes/1TB/marklab/runs/results-cellvit-bayesian-v1`. All six adjusted integrations and 46/46
  Bayesian library tests pass. Workspace warning-denied all-target/all-feature Clippy passed in
  `15m20s`; workspace no-default compilation, all-feature doc tests, strict all-feature docs,
  formatting, four-source Python syntax compilation, and whitespace checks pass. The documented
  full-integration/Nextest macOS loader loop was not rerun.
- This closes the current complete-covariate adjusted-count promotion gate, not BAY-03 or BAY-HIER-A.
  The next real hierarchy caller is a bounded audit of repeated CellViT slide counts nested within
  identified patients; only adequate within-patient replication may justify a patient random effect.
  Ordinal/hurdle, crossed/random-slope, CmdStan, actual GPU, and causal evidence remain absent.

## Canonical CRC spatial-fingerprint result checkpoint 69 — 2026-08-27

- Sealed the real-data CRC result bundle after bounded admission of 169 TCGA CRC CellViT patients,
  1,836 nonoverlapping provenance-complete fields, 442,979 admitted cells, raw 1,280-dimensional
  cell vectors, exact physical scales, molecular labels, and patient/site identities. Four
  provenance-sorted fields per patient were used where available for stability; field diagnostics
  ran in six bounded shards and were hashed rather than routed through exhaustive serial durable
  execution. Patient-held-out and site-held-out retrieval remained durable.
- M2 classical coordinate summaries are stable under 80% cell subsampling (patient-rank Spearman
  `0.920`–`0.953`) and adjacent 25–100 micrometre scales (`0.578`–`0.704`), while graph 2x2-versus-
  3x3 stability is only `0.270`–`0.609`. The complete M2 lane is null-to-negative relative to M0:
  patient-held-out top-1 changes by `-0.018` and patient-label energy/MMD p-values are
  `0.2175/0.295`.
- Projection-free M3 raw CellViT summaries pass the prespecified bounded stability gate: median
  feature-rank Spearman is `0.826` for full-source versus four-field summaries, `0.968` under 80%
  cell subsampling, and `0.990` against leave-one-field summaries. Their patient-held-out top-1
  increment is `+0.012` with an interval spanning zero; energy/MMD p-values are `0.125/0.1935`.
- M4 input review caught and corrected a false-neighbor risk before scientific use: separate slide
  field frames are translated 1,000 micrometres apart while all within-field distances are
  preserved, so no cross-field pair can enter the 0–100 micrometre analysis. Intermediate/far raw-
  vector variograms pass cell/field stability, but the near band has only 87 patients with two
  eligible fields. The full M4 lane reduces patient-held-out top-1 by `-0.141`; energy/MMD are null.
- M6 fuses only stability-admitted components, never components selected for molecular association:
  M0, four M2 classical-coordinate features, 55 M3 raw nonspatial features, and the intermediate/
  far M4 variograms. It admits 166 patients and 67 features. Patient-held-out top-1 changes by
  `+0.012` and site-held-out top-1 by `-0.018`, both uncertain; patient-label energy/MMD p-values are
  `0.33/0.2335`. A separate process replayed M6 byte-identically as a hit without adding a ledger
  row, as also proved for M2, M3, M4, and Schürch CODEX.
- Orthogonal Schürch CODEX validation uses 34 MSI/MSS patients, 136 cores, 240,554 provenance-
  retained cells and nine declared neighborhood fractions; it never treats CODEX proteins and
  CellViT vectors as one raw feature space. The neighborhood fingerprints are stable under 80%
  cell subsampling and core resampling, but balanced top-1 is `0.400`, MSI top-1 recall is zero,
  and energy/MMD p-values are `0.914/0.659`. Existing independent Schürch H&E CellViT evidence is
  retained as direction-generating, not as a direct 67-feature transfer.
- M7 admits the existing exact pinned PyMC 6.3.0 CPTAC repeated-slide hierarchy: 105 patients,
  217 slides, 81 repeated patients, MSI/MSS and sex effects, zero divergences, `R-hat=1.00232`, and
  byte-identical durable miss/hit outputs. Its marginal MSI-minus-MSS probability difference is
  `-0.0792` with interval `[-0.1569, 0.000094]`. A distinct ROI-within-slide level and cohort effect
  remain unavailable because this typed caller contains one cohort and no separate ROI level.
- The canonical bundle at
  `/Volumes/1TB/marklab/runs/results-crc-spatial-fingerprint-v1/bundle` contains 29,087 patient-
  feature rows, 4,969 specimen-feature rows, full similarity/prediction/diagnostic references,
  Bayesian and cross-cohort summaries, seven exact unavailable-lane blockers, 183 artifact hashes,
  and nine explicit stability-output hashes. Its five files are mirrored byte-identically under
  `/Users/user/Bench/results-crc-spatial-fingerprint-v1/bundle`. The scientific result is null:
  technically reproducible blocks do not establish a recurring molecular-class fingerprint across
  independent patients or cohorts. No clinical, causal, prospective, or performance claim is made.

## Exact nearest/empty-space checkpoint 70 — 2026-08-27

- Added one complete user-facing `marklab nearest-space` and `marklab project nearest-space`
  workflow. It estimates event nearest-neighbour G, deterministic-probe empty-space F, and finite J
  on the existing exact polygon/multipolygon/hole observation window. Both distributions use the
  standard reduced-sample border rule; probes are fixed cell-centred rectangular-grid locations
  with exact spacing, retained count, window identity, and half-cell-diagonal displacement metadata.
- J is available only where F and G exist and `1-F` exceeds the caller's positive denominator
  floor. Empty/singleton inputs, no eligible centers, and unstable denominators remain typed; no
  infinity or non-finite result can enter the strict `marklab.nearest_space` version-one document.
- The shared `SpatialGeometryPlan2D`/`SpatialIndex2D` now owns both K/L pair geometry and F/G/J
  nearest queries. Conditional CSR resamples the whole location pattern, reuses the exact fixed
  probe plan and window, namespaces its seed independently, and supplies separate F, G, and J ERL
  envelopes. Explicit point/radius/probe/query/draw/memory ceilings cover observed and null work.
- Hand-computable, independent brute-force, and pinned SciPy 1.18.1 `cKDTree` fixtures agree on
  every F/G/J count and value. Deterministic null calibration exercises 80 F/G component tests;
  clustered and inhibited controls have the expected J directions. A separate process replays the
  exact durable result as a hit without adding a ledger row, while changing the seed creates a new
  cache key and one new execution.
- PP-04/PP-04A/PP-04B/PP-04C are complete. FND-03 and WS-22 advance to active rather than blocked;
  compartment partitions, volume windows, pair-correlation g, inhomogeneous/cross/marked methods,
  and broader external geometry agreement remain. The next immediate geometry/mark caller is a
  categorical mark-connection/cross-type workflow over the existing typed MarkTable, not a generic
  mark or workflow registry.

## Typed categorical pair checkpoint 71 — 2026-08-27

- Added one exact `categorical_mark_connection_cross_k` production workflow over the existing
  provenance-validated `histologic_compartment` MarkTable column and physical framed window. A
  caller declares two distinct ordered level labels and a physical radius axis; unknown, duplicate,
  empty, or provenance/frame-incompatible inputs fail before a result.
- One bounded directed pair plan is constructed exactly once and reused for observed labels and
  every null assignment. Each shell reports the directed source-to-target mark-connection
  probability among all boundary-eligible directed pairs; each cumulative radius reports
  standard-border directed cross-K using exact eligible source centers and target count. The hand
  line oracle is `1/6` for connection and `7.0` for cross-K at 1.1 micrometres.
- Random-label inference consumes the existing `InferenceDesign`: complete categorical rows move
  across fixed locations, all category counts and geometry remain fixed, and connection/cross-K
  receive separate two-sided ERL families. Point, radius, retained-pair, permutation-pair, and
  memory ceilings include retained geometry, null matrices, indices, and work buffers.
- The typed durable node verifies every MarkTable provenance artifact through the existing store,
  encodes a strict canonical result, and reopens as a hit without another execution; changing the
  seed produces a distinct miss. An alternating-label control increases both connection and cross-K
  over the prespecified segregated control.
- The categorical portion of MRK-01A and the first homogeneous PP-03B cross-K specialization are
  complete, while both tracker families remain active: probability/simplex connection, general
  multitype import, cross-g, inhomogeneous intensity, many-pair multiplicity, and real CellViT-to-
  MarkTable CLI/import evidence remain. The next immediate caller is probability-weighted binary
  mark connection using the existing dense probability column.

## Expected probability-pair checkpoint 72 — 2026-08-27

- Added `probability_mark_connection` for one exact dense binary-probability MarkTable column. Its
  named shell estimand is `sum p_i p_j / eligible directed pairs`: the conditional expected
  positive-positive connection under independent Bernoulli label uncertainty, without sampling or
  thresholding latent labels.
- The global random-label expectation is the exact ordered without-replacement probability-row
  mass `((sum p)^2 - sum p^2) / (n(n-1))`. Complete `f32` probability rows move across fixed
  locations through the existing `InferenceDesign`; one two-sided ERL family is evaluated on the
  same standard-border directed pairs. Zero expected positive-positive pair mass and zero effective
  negative mass fail explicitly rather than producing an uninformative promoted curve.
- Categorical and probability connection now consume the same private retained pair-plan owner.
  The extraction preserves the original categorical pair-plan digest domain, geometry ordering,
  edge eligibility, and resource behavior; all categorical focused regressions pass unchanged.
- The four-point hand case `[1, 0.5, 0, 1]` has shell contribution mass `1`, six eligible directed
  pairs, connection `1/6`, and random-label expectation `1/3`. A clustered binary-probability
  control exceeds its alternating counterpart. Missing mark identity, rare mass, all-positive mass,
  and one-short pair ceiling fail through typed errors.
- The store-aware durable node binds exact pattern, MarkTable/provenance, window, mark ID, expected-
  contribution mode, radii, seed, null controls, and limits. A separate reconstruction reopens as a
  hit with one execution; a seed change produces the second miss. MRK-01A is complete. MRK-01B is
  now active for normalized continuous mark correlation over the already-typed finite continuous
  column; probability simplex and sampled-label uncertainty remain under MRK-02C.

## Normalized continuous mark-correlation checkpoint 73 — 2026-08-27

- Added `continuous_mark_correlation` for the existing exact positive finite
  `nucleus_area_um2` MarkTable column. Each standard-border shell reports the compensated `f64`
  mean of directed `m_i m_j` products divided by one global arithmetic mark mean squared. The mean
  is fixed over all admitted rows and is never radius-, edge-, or permutation-specific.
- The exact finite-row random-label expectation is
  `(((sum m)^2 - sum m^2) / (n(n-1))) / global_mean(m)^2`. Complete continuous rows move through
  the existing `InferenceDesign` over one retained geometry plan, with one two-sided ERL family.
  Constant marks, missing identity, pair work, permutation work, and retained bytes fail explicitly.
- The four-point line `[1,2,3,4]` has global mean `2.5`, population variance `1.25`, six directed
  adjacent pairs with product sum `40`, shell correlation `16/15`, and random-label expectation
  `14/15`. A standalone direct-pair Python oracle regenerates its fixture byte-for-byte and agrees
  with Rust at exact counts and declared `f64` tolerance; adjacent ordered marks exceed the
  prespecified alternating control. An adversarial dominant-positive-mark case retains nonzero
  expected random-label mass through a cancellation-resistant prefix-product accumulation.
- Direct review found that the prior categorical/probability retained-byte preflights counted stored
  null curves but not internal ERL matrix copies and rank/depth/order workspaces. One shared checked
  estimator now covers those peak allocations plus actual retained directed-pair vector capacity
  for all three workflows. A limit one byte below each workflow's reported total fails before
  inference; all affected categorical/probability regressions remain green.
- The strict store-aware node binds mark/provenance, unit, frame/window, normalization, radii, null
  controls, seed, limits, and implementation identity. A separately reconstructed durable project
  hits with one execution; a seed change misses. MRK-01B is complete. MRK-01C mark-weighted K is the
  next immediate caller and must remain a distinct cumulative estimand.

## Cumulative mark-weighted K checkpoint 74 — 2026-08-27

- Added `continuous_mark_weighted_k` as the third and distinct MRK-01 family. Its pair weight is
  `m_i m_j / global_mean(m)^2`; cumulative weighted K is `area * sum(weight) /
  (point_count * standard-border eligible centers)`. Every radius also reports unweighted K from
  the identical directed pairs and denominator, so spatial geometry is never hidden inside the
  mark effect.
- The global positive-mark mean and cancellation-resistant finite-row random-label weight are
  reused from MRK-01B. Complete continuous rows move over the fixed retained geometry, and only the
  weighted-K curve receives a two-sided ERL family. Constant marks, missing identity, pair/null work,
  no eligible centers, and one-byte-short retained memory remain explicit typed states or failures.
- On the four-point line `[1,2,3,4]` at 1.1 micrometres, six cumulative directed pairs have product
  sum `40`, normalized weight sum `6.4`, unweighted K `10.5`, weighted K `11.2`, and exact
  random-label weighted-K expectation `9.8`. An independent direct-loop Python fixture regenerates
  byte-for-byte and agrees with every Rust count/value at declared tolerance. Adjacent ordered marks
  raise weighted K relative to the alternating control while leaving unweighted K exactly fixed.
- The strict durable node binds mark/provenance, unit, frame/window, weight and edge formulas, radii,
  null controls, seed, limits, implementation identity, and a closed result codec. A separately
  reconstructed project hits with one ledger execution; changing the seed misses. MRK-01,
  MRK-01A, MRK-01B, and MRK-01C are now complete.
- Final finite-result review closed an inherited K-family boundary: finite radii that would overflow
  `pi r^2` are now rejected by unmarked K/L, categorical cross-K, and mark-weighted K configuration
  before geometry or output construction.

## Typed spatial-workflow stabilization checkpoint 75 — 2026-08-27

- The four related typed mark workflows from checkpoints 71–74 now pass one major-checkpoint
  workspace stabilization. All-feature warning-denied Clippy found one pre-existing complex tuple
  return in durable region retrieval; the focused cleanup removed a redundant standardized-matrix
  construction while preserving its mean/scale validation and decoded-result behavior.
- Workspace formatting, warning-denied all-target/all-feature Clippy, no-default compilation, and
  all-feature workspace doctests pass. The focused durable region-retrieval integrations pass after
  the cleanup.
- One fresh serial all-feature workspace test attempt compiled successfully in 16m50s. The main
  library passed 294 tests with 21 ignored, the CLI unit target passed 3/3, and `api_contract`
  passed 6/6. The run was then stopped after the next integration binary reproduced the documented
  per-binary macOS loader-verification delay; it was not retried and is not represented as a full
  workspace-test pass.
- Tracker re-evaluation promotes only homogeneous PP-03A pair-correlation to active. The next
  production milestone remains a distinct compact-support standard-border g/cross-g result with
  CSR and random-label calibration; inhomogeneous variants still require PP-02 intensity ownership.

## Homogeneous pair-correlation checkpoint 76 — 2026-08-27

- Added distinct unmarked `homogeneous_pair_correlation` and typed categorical
  `categorical_cross_pair_correlation` result families. Both freeze one explicit physical bandwidth,
  the positive-support Epanechnikov kernel, and standard-border center eligibility at `r+h`; neither
  aliases shell counts, centered mark covariance, cumulative K, or cross-K.
- Unmarked g is `area * sum(k_h(r-d_ij)) / (2*pi*r*n*eligible_centers)`. On a three-point
  100-square-micrometre window, two directed unit-distance pairs contribute kernel mass 3 and
  g `5.305164769729845`. Whole-pattern conditional CSR supplies its two-sided ERL family.
- Directed categorical cross-g replaces `n` with the total target count while retaining exact
  source-center border eligibility and declared level order. The four-point tumor-to-stroma case
  has one supported pair, kernel mass 1.5, and cross-g `1.6711269024649011`; alternating labels
  increase the prespecified result over segregated labels. Complete categorical rows move under
  random labeling without rebuilding the retained geometry.
- Independent standard-library Python pair loops regenerate both fixtures byte-for-byte. Typed
  empty kernel support, invalid/overflowing radii, unknown levels, compensated finite accumulation,
  exact configuration/mark/frame identity, one-byte-short memory, and bounded pair/null work are
  enforced. Each strict durable node reopens as a verified hit with one ledger execution and a seed
  change creates one new miss.
- PP-03A and PP-03B each gain one bounded homogeneous specialization but remain active pending
  pinned external `pcf`/`pcfcross` agreement, broader null/edge calibration, general multitype
  coverage, and inhomogeneous intensity. The next consumed workflow is explicit cross-fitted
  intensity feeding inhomogeneous K/L, not a generic estimator registry.

## Leave-one-out inhomogeneous K/L checkpoint 77 — 2026-08-27

- Added one complete PP-05→PP-02 production flow. A caller fixes a physical Gaussian bandwidth and
  deterministic cell-centred integration grid. Every observed event receives a positive finite
  intensity from all other events, scaled by `n/(n-1)` and divided by the quadrature-estimated
  kernel mass inside the exact polygon/multipolygon/hole window. No event contributes to its own
  intensity and no bandwidth is learned from the reported K/L curve.
- The typed intensity artifact retains every event row/boundary mass/training count plus every fixed
  probe center, intensity, cell mass, total mass, grid spacing/displacement, and exact digest used by
  the null. Standard-border K is the compensated ratio of eligible ordered-pair
  `1/(lambda_i lambda_j)` mass to eligible-center `1/lambda_i` mass; it reduces exactly to the
  existing homogeneous reduced-sample estimator under constant intensity. L remains `sqrt(K/pi)`.
- The fixed gridded pilot draws whole conditioned location patterns with uniform in-cell jitter
  restricted to the exact window; null events are weighted by that frozen observed pilot rather
  than refitting it. One two-sided L-curve ERL family reports the descriptive within-pattern null.
  Twenty deterministic gradient inhomogeneous-Poisson controls show no gross anti-conservatism, and
  a prespecified tight-cluster control retains excess short-range K after reweighting.
- A standalone Python direct loop agrees on four boundary masses, four leave-one-out intensities,
  eligible centers/pairs, inverse-intensity sums, K, and L. Polygon-hole probe exclusion, singleton,
  near-zero intensity, invalid finite output, and one-short memory/intensity/pair/null-draw limits
  are explicit. The durable node uses a private exact-f64-bit codec after ordinary JSON numbers were
  proven to alter grid digests; a reconstructed project hits with one execution and seed changes
  miss.
- This closes one bounded Gaussian/cell-quadrature specialization, not the PP-02/PP-05 families.
  Continuous exact polygon-kernel integration, multiple/piecewise estimators, automatic bandwidth,
  broader calibration, pinned `spatstat` agreement, compartments, and real scale evidence remain.

## Durable inhomogeneous pair-correlation checkpoint 78 — 2026-08-27

- Added one inhomogeneous Epanechnikov pair-correlation workflow that consumes exactly the persisted
  Gaussian leave-one-out event and fixed-grid intensity artifact already owned by K/L. Its explicit
  pair-smoothing bandwidth is separate from the intensity bandwidth, every center uses standard-
  border eligibility at `r+h_pair`, and the inverse-intensity kernel/center ratio reduces to the
  homogeneous normalization under constant intensity.
- The observed pilot is fitted once and frozen across the existing conditioned gridded null. The
  result retains typed empty-support states, one two-sided ERL family, exact intensity/pair/null work
  telemetry, and hard memory ceilings. A direct Python loop gives g `2.064293021119719`; twenty
  gradient-null controls stay below the prespecified gross anti-conservatism ceiling, and a matched
  short-range clustered control has g above one.
- The strict durable node shares only the exact-f64 codec and pilot validation demonstrably used by
  K/L and g. A reconstructed project returns a byte-identical hit with one ledger row, while a seed
  change creates a second miss. No bandwidth selector, estimator/kernel registry, multitype
  inhomogeneous expansion, or pinned `spatstat` agreement is claimed.

## Exact binary compartment-interface checkpoint 79 — 2026-08-27

- Added one physical-frame-bound binary compartment partition over the existing exact polygon/
  multipolygon/hole window owner. Negative and positive compartment windows must tessellate the
  analyzed domain with aligned canonical segments and area agreement within sixteen f64 ULPs. Every
  outer segment belongs to exactly one compartment and every internal segment has one exact mate;
  gaps, overlaps, frame drift, representation drift, absent interfaces, and excessive segment work
  fail without snapping, repair, overlay inference, or rasterization.
- Only matched internal segments enter the interface R-tree. Signed distance is positive on the
  declared positive side, negative on the negative side, exact zero on the shared interface, and
  unavailable outside the analyzed window. A tissue-edge point therefore retains its distance to
  the biological interface rather than incorrectly becoming zero. GEOS 3.14.1 independently agrees
  on compartment/domain areas, union, shared line, interface length, and all declared distances.
- The immediate typed caller consumes the existing row-aligned `histologic_compartment` MarkTable.
  It retains stable CellIds, row labels, per-cell signed micrometre distances, two compartment
  summaries, exact provenance/configuration/partition identities, and hard point/query/memory
  bounds. Non-interface annotation/geometry disagreement fails. A reconstructed durable project
  returns the identical profile as a hit with one execution; changing only the query ceiling misses.
- This is a descriptive per-specimen binary specialization. It does not treat cells as patient
  replicates or claim multiclass/residual/uncertain partitions, contact fractions, phenotype
  inference, patient effects, real pathology validation, clinical use, or result-format changes.

## Exact compartment-contact checkpoint 80 — 2026-08-27

- Added the immediate GEO-01B caller over the exact binary partition. For each negative/positive
  role it separately retains shared internal-interface length, analyzed-tissue outer-boundary
  length, and the complete compartment polygon boundary. Contact fraction is fixed as shared over
  complete boundary; it cannot silently substitute a cell-count, tissue-window, convex-hull, or
  shared-only denominator.
- The partition now reconstructs each canonical compartment perimeter from deterministically sorted,
  compensated shared and outer segment sums and rejects disagreement beyond sixteen f64 ULPs. GEOS
  3.14.1 independently reports 10 micrometres shared, 20 outer, and 30 complete boundary for each
  rectangle, giving contact `1/3` for both roles.
- A strict separate durable node reopens as an identical hit with one execution. Swapping the
  positive and negative compartment roles changes the partition/cache identity and creates a miss
  while preserving the exact geometry. This is geometric compartment contact, not phenotype/cell
  contact, a multiclass matrix, uncertain segmentation, patient inference, or biological evidence.

## Exact compartment fragmentation and cell-mixing checkpoint 81 — 2026-08-27

- Added vector-polygon fragmentation on the exact binary partition. Canonically sorted component
  areas drive component/hole counts, largest-component area fraction, Shannon area entropy in nats,
  entropy normalized by `ln(component_count)`, and complete perimeter/area in inverse micrometres.
  Two islands of areas 1 and 4 have largest fraction `0.8` and normalized entropy
  `0.7219280948873623`; the 95-square-micrometre background is one component with two holes. GEOS
  3.14.1 independently agrees on all areas, perimeters, union, and derived ratios.
- Added the distinct cell-mixing estimand only after declaring a typed cell table and one fixed
  physical adjacency radius. The canonical spatial index produces exact undirected edges while
  charging all directed visits. The four-cell 2.1-micrometre control has three edges, one cross edge,
  observed cross fraction `1/3` versus complete-random-label expectation `2/3`, and normalized
  same/cross edge entropy `0.9182958340544894`; role-specific neighbor incidences agree exactly.
- Fragmentation and cell mixing retain separate result types and durable nodes. Reconstructed
  projects hit with one execution; orientation or radius changes miss. Empty graphs, annotation/
  geometry disagreement, and one-short point/query/pair/memory work fail explicitly. Neither
  component areas nor edges are treated as patient replicates, and no scale was selected from the
  result.
- This closes bounded binary vector-component and fixed-radius mixing specializations, not
  multiclass, multiscale stability, uncertain segmentation, real-mask validation, patient inference,
  clinical evidence, or result-format changes.

## Compartment-geometry stabilization checkpoint 82 — 2026-08-27

- Stabilized the four related exact binary compartment workflows from checkpoints 79–81: oriented
  typed-cell interface profiles, explicit-denominator contact, vector-component fragmentation, and
  fixed-physical-radius cell mixing. Workspace formatting, all-target/all-feature warning-denied
  Clippy, no-default compilation, all-feature doctests, and strict all-feature workspace docs pass.
- All 13 focused geometry tests and three independent fixture regenerations remain green. The
  canonical CRC fingerprint bundle was not recomputed; a read-only hash audit instead reconfirmed
  all 183 source artifacts, nine stability hashes, four generated hashes, and byte-identical local/
  Mac-mini five-file bundle copies.
- The full workspace integration suite and Nextest were not rerun because the active instruction
  forbids retrying the documented macOS binary-loader verification loop. No feature matrix,
  benchmark, fuzz, memory, packaging, dependency-audit, push, publication, deployment, or history
  rewrite was run.

## Typed probability-simplex composition checkpoint 83 — 2026-08-27

- Added one contiguous CellViT-compatible probability-simplex MarkTable column. It binds ordered
  unique class labels, morphology-prediction status, exact provenance, row/class/value identity,
  and complete f32 rows. Every value must be finite in `[0,1]`, every row has exact class width, and
  the supplied row sum must be within `1e-5` of one; values are preserved without renormalization,
  thresholding, sampling, or coordinate-wise movement.
- The immediate soft-composition caller reports class mean probabilities, mean per-cell Shannon
  entropy, aggregate composition entropy, effective class count, and maximum row-sum error under
  hard point/class/value/memory ceilings. The independent four-row Python oracle gives means
  `[0.375, 0.1875, 0.4375]`, mean row entropy `0.31387058129468837`, aggregate entropy
  `1.0433534269422904`, and effective class count `2.8387205126507515`.
- Exact MarkTable identity/provenance now includes the class codebook, row count, and all contiguous
  f32 bits. The store-aware durable node reopens as an identical hit with one ledger execution;
  changing only a resource ceiling misses. Existing scalar, declared-analysis, and typed Moran
  workflows pass unchanged.
- This is descriptive per-specimen soft composition, not probability calibration, hard-label
  replacement, patient inference, spatial neighborhood composition, clinical evidence, or a generic
  mark registry. Result-format 0.3 remains unchanged.

## Fixed-radius soft neighborhood checkpoint 84 — 2026-08-27

- Added the immediate spatial consumer of complete probability-simplex rows. A caller fixes one
  positive physical radius and the canonical geometry plan supplies deterministically sorted exact
  neighbors. Every focal cell retains stable CellId, neighbor count, and the arithmetic mean of
  complete target-neighbor simplex rows; zero-neighbor cells retain a typed unavailable vector.
- The result also reports directed pair visits and the directed-incidence-weighted aggregate class
  mass. The independent four-cell Python loop has two directed visits, two zero-neighbor cells,
  focal means `[0,1]` and `[1,0]`, and aggregate mass `[0.5,0.5]`. An all-isolated radius returns
  four typed unavailable rows and no aggregate vector rather than fabricated zeros.
- Exact table/provenance/window/radius identities and hard point/class/value/pair/memory ceilings
  are cache-bound. One-short pair and memory work fail. A reconstructed store-aware project returns
  an identical hit with one execution; changing only the physical radius misses.
- This is descriptive within-specimen soft neighborhood composition. No class probability is
  thresholded or renormalized, no radius is selected from the result, and cells/edges are not
  treated as patient replicates. Prespecified multiscale stability, patient inference, niche
  discovery, real validation, and result-format changes remain.

## Prespecified multiscale soft-neighborhood checkpoint 85 — 2026-08-28

- Added the bounded NIC-01A continuation over a strictly increasing caller-supplied physical-radius
  list. One canonical geometry/index plan is built and reused across every scale; each scale retains
  complete per-cell simplex means, stable CellIds, exact neighbor counts, typed zero-neighbor states,
  directed-incidence aggregate class mass, and an exact graph digest.
- Total directed visits are charged across all scales and adjacent available aggregate vectors
  report total-variation distance without selecting or ranking radii. The independent Python loop
  gives six total visits at 1.5/2.5 micrometres, second-scale mass `[0.375,0.625]`, and adjacent-scale
  distance `0.125`; two all-isolated scales retain an unavailable distance rather than zero.
- Exact table/provenance/window/radius-list/limit identity and point/class/radius/value/pair/memory
  ceilings are durable. A reopened project returns byte-identical fixed and multiscale hits without
  new executions; changing both radius contracts creates exactly two misses. Inconsistent retained
  row counts are rejected by the result codec.
- This completes one bounded prespecified NIC-01A multiscale specialization. Real patient/ROI
  stability, scale transport, niche discovery, population inference, and result-format changes are
  not claimed. NIC-01 remains active.

## Row-bound CellViT vector-artifact checkpoint 86 — 2026-08-28

- Added the first concrete `VectorArtifactRef` MarkTable column over the existing verified
  `CellEmbeddingArtifact`. Construction uses the materialized table only to prove exact QC, shape,
  logical-table, and ordered CellId agreement, then retains metadata and a row-identity digest; the
  matrix, row link, provenance graph, artifact store, and Arrow/Parquet codecs remain single-owned.
- The column binds stable mark ID/label, morphology-prediction status, modality, embedding-vector
  unit, missingness, embedding/expected-cell/row-link/provenance artifact identities, dimension,
  dtype, QC counts, and logical digest into MarkTable identity. `NotPermitted` rejects any unavailable
  row; `Allowed` preserves missing-vector, extraction-failed, and QC-rejected states in the verified
  table without exposing filler components.
- The existing declared binary CellViT centroid statistic is the immediate production caller. It
  rejects a typed vector reference that differs from the separately supplied verified artifact.
  Its store-aware node now forms one exact semantic-input union when the typed table and existing
  explicit embedding boundary name the same verified artifacts; the typed path returns a miss then
  an identical hit without a second execution.
- This is one bounded FND-04/FND-05/MRK-02D specialization. Other vector statistics, canonical real
  source-import promotion, independent genuine patch tensors, broader vector interchange, and
  result-format changes remain active or unavailable as previously recorded.

## Patient population-independence design checkpoint 87 — 2026-08-28

- Routed the existing patient-level MMD and energy-distance workflows through the shared
  `marklab-cohort::InferenceDesign` under an explicit `PopulationIndependence` null. One complete
  patient group label is the permutation unit and the unblocked admitted patient set is the exact
  exchangeability block; neither fingerprints nor feature coordinates are split.
- Both methods retain their existing private seed namespaces and one-sided-high alternatives. The
  shared Fisher–Yates index schedule reproduces every former label vector exactly, so observed
  statistics, null replicates, plus-one p-values, requested/attempted/completed counts, seeds, work
  bounds, CLI outputs, and the sealed CRC bundle remain unchanged.
- Independent slow references pass for linear/RBF biased/unbiased MMD and Euclidean energy distance.
  This is a bounded FND-06 population-independence increment, not blocked/stratified fingerprint
  inference, pairing, repeated/multisite exchangeability, multiplicity expansion, or a general null
  registry.

## Complete formal vector-mark admission checkpoint 88 — 2026-08-28

- Connected the existing probability–embedding and nucleus-area–embedding cross-covariance
  statistics to the same no-copy row-bound `VectorArtifactRef` already consumed by the binary
  centroid workflow. Each caller accepts the older direct declared input for compatibility, but when
  a typed vector mark is present it must name the exact separately supplied verified artifact.
- Changed logical content, physical artifact, provenance, row link, QC, dimension, or CellId order
  therefore cannot be substituted behind a typed mark. Probability values and measured positive
  square-micrometre nucleus areas remain separate declared scalar estimands; vector filler components
  remain inaccessible and non-present statuses retain their existing typed unavailable behavior.
- The three callers share only one private lookup on `DeclaredScalarPatternInput`; their distinct
  statistics, status rules, resource accounting, identities, and the centroid's store-aware codec/
  scheduler remain unchanged. The complete 64-test CellViT artifact graph passes.
- This completes the bounded MRK-02D canonical row-bound vector-admission prerequisite across its
  three immediate formal callers. FND-05/WS-24 remain active for canonical real source import,
  independent genuine patch vectors, and broader interchange; no vector registry, matrix copy, new
  physical format, or result-format change was added.

## Durable ordinal IHC composition checkpoint 89 — 2026-08-28

- Added one complete dense ordinal MarkTable specialization for a measured IHC caller. It binds a
  stable mark ID/label, ordered unique level codebook, exact zero-based row codes, measurement
  status, provenance, modality/unit/missingness, and every row value into table identity. Duplicate/
  malformed levels, invalid codes, non-IHC modality, unit drift, and multiple ordinal columns fail.
- The immediate descriptive composition reports level counts/proportions, inclusive cumulative
  proportions, lower/upper empirical median levels, Shannon entropy in nats, entropy normalized by
  `ln(level_count)`, and effective level count. It deliberately reports no code mean, variance,
  distance, or linear contrast because ordinal code spacing is not an interval scale.
- The independent Python oracle gives counts `[1,2,0,1]`, CDF `[0.25,0.75,0.75,1]`, median interval
  `weak`–`weak`, entropy `1.0397207708399179`, normalized entropy `0.75`, and effective count
  `2.82842712474619`. Point/level/memory ceilings are explicit.
- The durable node reopens as an identical hit with one execution and a limit-only change misses.
  Ordinary JSON decimals were proven to alter one entropy bit, so the exact finite-f64 codec already
  required by inhomogeneous K/L and g now has a crate-private shared owner. Both legacy durable
  workflows pass unchanged. This is bounded FND-04/IHC-01 coverage, not real IHC evidence,
  thresholding, proportional-odds inference, or result-format 0.3 change.

## Typed-mark and inference stabilization checkpoint 90 — 2026-08-28

- Stabilized checkpoints 83–89 across probability-simplex marks, fixed/multiscale soft
  neighborhoods, verified vector-artifact marks and all three formal callers, patient
  population-independence schedules, and ordinal IHC composition. Workspace formatting,
  all-target/all-feature warning-denied Clippy, no-default compilation, all-feature doctests, and
  strict workspace docs pass.
- Strict docs initially found one pre-existing `[0,1]` comment parsed as a broken link; rendering it
  as code fixed the concrete finding and the focused strict-doc rerun passed. No production or
  scientific result changed.
- The full workspace integration suite and Nextest were not rerun because the active instruction and
  checkpoints 51/52 prohibit retrying the macOS binary-verification loop. No feature matrix,
  benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or
  history rewrite was run.

## Blocked fingerprint population-inference checkpoint 91 — 2026-08-28

- Added exact patient-ID-keyed exchangeability blocks to the existing patient-level MMD and energy-
  distance workflows. Every admitted fingerprint must have one bounded block assignment; missing,
  duplicate, foreign, singleton-only, and group-confounded designs fail explicitly. Whole patient
  labels move only within lexically ordered exact blocks, using each method's unchanged private seed
  namespace, statistic, matrix, work limits, and plus-one one-sided-high p-value.
- `marklab cohort mmd` and `marklab cohort energy` now accept the optional trailing `block` CSV
  column and report `population_independence` plus the exact block count. The older four-column path
  retains its prior JSON shape. Independent slow references match all blocked replicates after
  reverse-order assignment input, and CLI coverage proves complete admission and conflicting-row
  rejection.
- This is a bounded FND-06/CMP-01C/CMP-01D/COH-01/WS-31/WS-34 increment. It enables prespecified
  site/batch-restricted fingerprint comparisons without treating features or cells as replicates;
  it does not infer blocks, residualize covariates, add pairing/repeated measures, or change the
  already sealed CRC result bundle.

## Blocked functional-curve inference checkpoint 92 — 2026-08-28

- Added the same exact patient-ID-keyed exchangeability blocks to the existing common-axis
  functional L2 permutation. Every complete curve remains attached to one whole patient label;
  neither axis coordinates nor values are split. The unblocked path now uses the shared
  `InferenceDesign` while reproducing its former private seed stream and numerical result exactly.
- `marklab cohort functional-permutation` accepts an optional trailing `block` column, rejects
  conflicting block declarations within a curve, and reports population independence plus exact
  block count only on the blocked path. Its legacy four-column JSON shape remains unchanged.
- An independent slow curve-level reference matches exact blocked p-values under reverse-order
  patient assignments. This advances FND-06/COH-01/WS-31/WS-34 for prespecified multiscale patient
  summaries; it does not register or smooth curves, infer blocks, select scales, residualize
  covariates, or add paired/repeated inference.

## Typed repeated-subject residual design checkpoint 93 — 2026-08-28

- Routed the existing repeated-measures Freedman–Lane workflow through an explicit
  `SubjectResidualSignSymmetry` inference design whose atomic permutation unit is one complete
  subject residual vector. Every visit for a subject therefore receives the same deterministic ±1
  sign; visits and residual coordinates never become independent population units.
- The typed design reproduces every former method-namespaced Rademacher sign exactly, so the reduced
  and full subject-fixed-effect models, target coefficient/standard error/statistic, two-sided
  plus-one p-value, work limit, and seed remain unchanged. The library result now retains the design,
  and the CLI reports its null family and permutation unit alongside the existing explicit residual-
  exchangeability assumption.
- This advances FND-06/COH-01/WS-31/WS-34 without asserting residual symmetry from the data. It does
  not add covariates, align visits, substitute a paired endpoint analysis, add cluster bootstrap
  weights, or generalize a resampling registry.

## Typed paired-patient sign-flip checkpoint 94 — 2026-08-28

- Routed the existing paired scalar workflow through the master-plan `PairedSignFlip` null with one
  complete condition-B-minus-condition-A patient difference as the atomic unit. Condition rows are
  never separated, and index permutation is explicitly unavailable for this sign design.
- The typed schedule reproduces the former private Rademacher stream exactly, preserving the
  studentized statistic, less/greater/equal-tail two-sided alternatives, plus-one p-values, limits,
  and deterministic CLI bytes. The library result retains the design; the CLI now reports its null
  family and complete-pair-difference unit.
- This advances FND-06/COH-01/WS-31/WS-34. It does not impute incomplete pairs, align repeated visits,
  add covariates or matching, or generalize a sign-flip registry.

## Patient inference-design stabilization checkpoint 95 — 2026-08-28

- Stabilized checkpoints 91–94 across blocked patient MMD/energy/functional inference, repeated-
  subject residual signs, and paired complete-difference signs. Workspace formatting, all-target/
  all-feature warning-denied Clippy, no-default compilation, all-feature doctests, and strict docs
  pass on the final production state.
- The full workspace integration suite and Nextest were not run because checkpoints 51/52 and the
  active instruction prohibit retrying the macOS binary-verification loop. No feature matrix,
  benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or
  history rewrite ran.

## Multisite patient-count overflow checkpoint 96 — 2026-08-28

- Multisite inference now rejects an overflowing cross-site patient total explicitly instead of
  panicking in debug builds or wrapping in optimized builds. Existing fixed/random-effects pooling
  and leave-one-site-out behavior is unchanged.

## Patient-level multisite contrast checkpoint 97 — 2026-08-28

- Added a complete patient-row multisite workflow. It requires globally unique patient IDs, exact
  site/group labels, finite endpoints, and at least two patients in each group at each site; a site
  missing either group fails rather than borrowing patients or treating lower-level rows as units.
- Each site reports exact group counts/means, group-A-minus-group-B effect, and Welch standard error,
  then reuses the existing fixed-effect or REML pooling, heterogeneity, prediction interval, and
  leave-one-site-out owner. The three-site hand oracle gives effect `3` and SE `1` at every site and
  pooled effect `3` over 12 patients.
- This advances COH-01/FND-06/WS-31/WS-34 without claiming site exchangeability, residualizing
  covariates, inferring site labels, or adding a generic meta-analysis framework.

## Canonical hierarchical-bootstrap ordering checkpoint 98 — 2026-08-28

- Patient-first hierarchical bootstrap now canonicalizes specimens by exact specimen identity
  within canonically ordered patients before applying its unchanged deterministic nested sampling
  stream. Equivalent CSV row reorderings therefore produce identical stored replicate means and
  percentile intervals instead of silently changing results.
- The independent slow reference and existing patient-first CLI remain green. This advances
  COH-01/FND-06 reproducibility without changing the specimen-row-mean estimand, hierarchy, seed,
  draw count, interval method, or treating specimens as population replicates.

## Typed hierarchical-bootstrap design checkpoint 99 — 2026-08-28

- Added the master-plan `HierarchicalBootstrap` null with one patient occurrence followed by its
  nested specimen draw as the atomic resampling unit. Exact canonical patient blocks/specimen counts
  are retained, flat index permutation is unavailable, and empty patient blocks fail explicitly.
- Both hierarchical-bootstrap and bootstrap-equivalence now consume and expose this typed design.
  The former SplitMix patient-first/nested-specimen stream, stored replicate means, percentile
  intervals, estimand, limits, and deterministic CLI outputs remain exact.
- This advances COH-01/FND-06/WS-31/WS-34 without inferring hierarchy, adding cluster weights or
  covariates, or treating specimens as population replicates.

## Whole-cluster patient inference checkpoint 100 — 2026-08-28

- Added cluster-randomized scalar inference from globally unique patient rows. Every cluster must
  retain one exact declared group; the workflow computes one equal-weight patient-endpoint mean per
  cluster and permutes complete cluster labels, so unequal cluster sizes cannot create patient-level
  pseudoreplication.
- The result exposes `ClusterLabelPermutation`, `CompleteClusterEndpoint`, patient/cluster counts,
  cluster-level group means, Welch effect/statistic, alternative, seed, and exact replicate counts.
  An independent slow oracle matches the p-value; duplicate patients, mixed-group clusters, and
  fewer than two clusters per group fail.
- This advances COH-01/FND-06/WS-31/WS-34 without inferring clusters, adjusting covariates, claiming
  intracluster-correlation modeling, or adding a generic cluster framework.

## Cohort hierarchy and cluster stabilization checkpoint 101 — 2026-08-28

- Stabilized checkpoints 97–100 across patient-row multisite contrasts, canonical hierarchical
  bootstrap ordering, typed patient-then-specimen draws, and whole-cluster label permutation.
  Workspace formatting, all-target/all-feature warning-denied Clippy, no-default compilation, all-
  feature doctests, and strict docs pass on the final production state.
- The full workspace integration suite and Nextest were not run because checkpoints 51/52 and the
  active instruction prohibit retrying the macOS binary-verification loop. No feature matrix,
  benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or
  history rewrite ran.

## Explicit randomized-interference design checkpoint 102 — 2026-08-28

- The existing randomized binary interference result now explicitly reports clustered-unit analysis,
  the randomized-interference fixed-outcome null, complete cluster assignment state as the atomic
  randomization unit, and exact unit/cluster counts.
- Assignment enumeration, within-cluster treated counts, graph/exposure mapping, HT/Hájek
  estimands, positivity, ChaCha20 stream, fixed-outcome test, limits, p-value, and prior output fields
  remain unchanged. The deterministic CLI and all causal library tests pass.
- This closes the concrete FND-06 interference design-summary gap without adding a cross-crate
  abstraction or claiming observational interference identification.

## Blocked single-step Max-T checkpoint 103 — 2026-08-28

- Single-step Max-T now accepts exact patient-ID-keyed exchangeability blocks while keeping each
  patient's complete endpoint vector atomic. The typed design records population independence,
  two-sided alternative, exact block count, endpoint family, alpha, and the unchanged method seed.
- Independent slow blocked and unblocked references match every adjusted p-value and critical value.
  The CLI accepts an optional trailing `block` column and emits blocked design fields only for that
  path, preserving legacy unblocked JSON shape.
- This advances FND-06/COH-01/CMP-01B/WS-31/WS-34 without endpoint selection, inferred blocks,
  covariate residualization, step-down testing, or a generic multiplicity registry.

## Patient-family step-down Max-T checkpoint 104 — 2026-08-28

- Added step-down Max-T to the existing complete patient endpoint family for both unrestricted and
  exact-block label designs. Each permutation remains whole-patient and shared across endpoints;
  exact observed-statistic ties form one step and adjusted p-values are monotone in decreasing
  observed absolute Welch order.
- The implementation streams one null endpoint vector at a time under the existing 100-million-
  evaluation ceiling. It preserves endpoint order, the initial complete-family critical value,
  alpha, seed namespace, block compiler, and legacy single-step CLI output.
- Independent slow unrestricted/blocked references match every adjusted p-value. Focused unit,
  reference, and CLI tests pass, as do affected warning-denied Clippy, package no-default
  compilation, strict package docs, affected-file formatting, and whitespace checks. A package-wide
  filtered test command was interrupted when Cargo began launching unrelated integration binaries;
  the named Max-T library and integration targets were then run directly.

## Paired endpoint-family Max-T checkpoint 105 — 2026-08-28

- Added `marklab cohort paired-max-t` and a typed library workflow for exact complete endpoint
  vectors observed under two declared conditions per patient pair. One deterministic sign is drawn
  per pair and applied to every endpoint together; the result explicitly records the complete
  patient-pair difference vector, two-sided null, complete-family multiplicity, condition labels,
  single-step or step-down correction, alpha, critical value, seed, and replicate counts.
- The implementation streams null endpoint vectors under a 100-million pair-by-endpoint-by-
  replicate ceiling. Incomplete pairs/families, duplicate condition endpoints, non-finite values,
  degenerate endpoint contrasts, and invalid alpha/permutation bounds fail rather than falling back
  to unpaired or endpoint-wise signs.
- An independent slow vector-sign oracle matches every adjusted p-value and critical value for both
  corrections. Focused paired-family, legacy Max-T, scalar-paired, inference-design, and CLI tests
  pass, as do affected warning-denied Clippy, package no-default compilation, strict package docs,
  affected-file formatting, and whitespace checks.

## One-covariate patient Freedman-Lane checkpoint 106 — 2026-08-28

- Added `marklab cohort covariate-permutation` and a typed library workflow for one independent
  patient outcome, binary group, and prespecified nuisance covariate. The reduced intercept-plus-
  covariate model owns fixed fits/residuals; each residual moves as one complete patient value; the
  full model tests the adjusted group-A-minus-group-B coefficient.
- The design records a covariate-conditional residual-permutation null, complete patient residual
  unit, alternative, exact reduced/full columns, residual degrees of freedom, deterministic
  covariate center/max-deviation scale, seed, and exact replicate counts. Duplicate patients,
  undeclared groups, non-finite/constant covariates, rank deficiency, undefined statistics, and work
  above 100 million patient-replicate evaluations fail.
- An independent Frisch-Waugh-Lovell oracle matches the coefficient, standard error, statistic, and
  exact p-value; reversed rows and a `1e100` covariate rescaling preserve inference, while exact
  group-covariate collinearity fails. Focused new/legacy CLI tests, warning-denied Clippy, package
  no-default compilation, strict package docs, affected formatting, and whitespace checks pass.

## Blocked one-covariate residual permutation checkpoint 107 — 2026-08-28

- The one-covariate patient Freedman-Lane workflow now accepts exact patient-ID-keyed blocks and
  permutes each complete reduced-model residual only within its declared block. Reverse-ordered
  assignments canonicalize to patient order; missing, duplicate, foreign, invalid, or entirely
  singleton block designs fail through the shared exact block owner.
- The CLI accepts one optional trailing `block` column and reports `blocked=true` plus exact block
  count only on that path. The unblocked version-one JSON shape and permutation stream remain
  unchanged. An independent two-block restricted-shuffle oracle matches the exact p-value.
- The complete covariate CLI passes 2/2; covariate references pass 3/3; affected MMD 3/3, energy
  2/2, functional 2/2, and Max-T 3/3 references pass after patient-block alignment reuse. A first
  command named the nonexistent `functional_reference` target and failed before that step; the
  canonical `functional_permutation_reference` target then passed. Affected Clippy, no-default,
  strict docs, formatting, and whitespace checks pass.

## Inference-family stabilization checkpoint 108 — 2026-08-28

- Stabilized checkpoints 102–107 across explicit randomized interference, blocked/single-step/
  step-down/paired Max-T, and unblocked/blocked one-covariate patient residual inference.
  Workspace formatting, warning-denied all-target/all-feature Clippy, no-default compilation,
  all-feature doctests, and strict all-feature docs pass on the final production state.
- The full workspace integration suite and Nextest were not run because checkpoints 51/52 and the
  active instruction prohibit retrying the macOS binary-verification loop. No feature matrix,
  benchmark, fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or
  history rewrite ran.

## Named nuisance-matrix Freedman-Lane checkpoint 109 — 2026-08-28

- Added `marklab cohort covariate-matrix-permutation` and typed unblocked/blocked library workflows
  for 1–32 exact ordered nuisance columns per independent patient. The reduced model contains only
  intercept plus the fixed nuisance matrix; the full model adds one group indicator; complete
  patient residuals move globally or only within exact patient-ID blocks.
- Every nuisance name, center, max-deviation scale, model dimension, residual degree of freedom,
  group identity, alternative, seed, block state, and replicate count is explicit. Missing,
  duplicate, non-finite, constant, inconsistent, or rank-deficient columns fail. A 100-million
  OLS-work ceiling accounts for patient rows, squared/cubic column work, and all requested fits.
- An independent modified-Gram-Schmidt/FWL oracle matches coefficient, standard error, statistic,
  and exact unblocked/blocked p-values. Patient-order and extreme per-column rescaling are invariant;
  missing and collinear columns fail. Matrix references pass 2/2, the matrix CLI 1/1, legacy scalar
  covariate references 3/3 and CLI 2/2, plus affected Clippy/no-default/strict-doc/format checks.

## Adjusted multisite patient contrast checkpoint 110 — 2026-08-28

- Added `marklab cohort multisite-covariate-contrast` and a typed library workflow that fits the
  same exact 1–32-column nuisance design independently within every site. Each site reports its
  adjusted group coefficient, OLS standard error, residual degrees of freedom, patient counts, and
  site-specific centers/scales before the existing fixed-effect or REML pool runs.
- Patient IDs are globally unique and site rows canonicalize by patient ID. The independent oracle
  initially exposed order-dependent floating accumulation; canonicalization now makes reversed-row
  results exact. Incomplete matrices, non-finite/constant columns, fewer than two patients per group,
  site-specific rank deficiency, invalid site counts, and work above 100 million units fail.
- Independent modified-Gram-Schmidt/FWL site effects/SEs and inverse-variance pooling agree. The
  adjusted multisite reference passes 2/2, its CLI 1/1, and legacy multisite CLI 2/2. The first
  warning-denied Clippy run found one range-loop warning; the corrected final-state package/CLI
  Clippy, no-default, strict docs, formatting, and whitespace checks pass.

## Covariate-adjusted whole-cluster inference checkpoint 111 — 2026-08-28

- Added `marklab cohort cluster-covariate-permutation` and a typed library workflow that first
  reduces every exact cluster to equal-weight patient outcome and nuisance-column means. The
  reduced intercept-plus-1–32-column model is fixed at the cluster level; complete cluster
  residuals move through a private deterministic namespace; the full model tests one adjusted
  cluster-level group-A-minus-group-B coefficient.
- The result records the cluster analysis level, conditional residual null, complete-cluster
  residual unit, patient/cluster/group counts, exact names/transforms/model dimensions, residual
  degrees of freedom, alternative, seed, and exact replicate accounting. Duplicate patients,
  mixed-group clusters, incomplete/non-finite/constant/collinear matrices, insufficient independent
  clusters, undefined statistics, and work above 100 million OLS units fail.
- An independent modified-Gram-Schmidt/FWL oracle matches coefficient, standard error, statistic,
  and exact residual-permutation p-value over hand-computed unequal-size cluster summaries.
  Reversed patient rows and extreme per-column rescaling preserve inference. New references pass
  2/2, the CLI passes 1/1, legacy cluster and nuisance-matrix references pass 3/3, and their CLIs
  pass 2/2. Warning-denied affected package/root Clippy, package no-default compilation, strict
  package docs, affected formatting, and whitespace checks pass. Real effect validation remains
  unavailable because no admitted user-authorized CRC dataset declares randomized treatment groups
  at the cluster assignment unit.

## Ordered endpoint-family gatekeeping checkpoint 112 — 2026-08-28

- Added `marklab cohort hierarchical-max-t` and `hierarchical_gatekeeping_max_t` for two or more
  prespecified ordered endpoint families. One deterministic whole-patient label schedule feeds each
  local complete-family single-step or step-down Max-T test; the first family is open, and its
  successor opens only when every endpoint in the current family is rejected at the declared
  family-wise alpha.
- Exact family order/names/members, patient/group counts, shared population-independence null,
  patient-label unit, local correction/critical values/p-values, gated decisions, opening rule,
  seed, alpha, and replicate counts are explicit. Families must exactly partition every patient's
  complete endpoint vector under a 100-million patient-by-endpoint-by-permutation ceiling. Closed
  families retain local diagnostics but cannot report rejected endpoints.
- Independent slow label-shuffle oracles match every local critical value, single-step and
  step-down adjusted p-value, opened-family count, and gated decision. A primary non-rejection
  closes the secondary family even when its local diagnostic is small; overlap, incomplete
  partitions, and fewer than two families fail. New references pass 2/2 and CLI 1/1; legacy Max-T
  references pass 5/5 and CLIs 4/4. Affected warning-denied package and root-binary Clippy,
  no-default compilation, strict docs, formatting, and whitespace checks pass. Real CRC execution
  remains unavailable because no admitted artifact contains a prospectively prespecified ordered
  endpoint-family hierarchy; no hierarchy was selected after viewing outcomes.

## Adjusted and hierarchical inference stabilization checkpoint 113 — 2026-08-28

- Stabilized checkpoints 109–112 across named nuisance-matrix patient inference, adjusted
  within-site pooling, covariate-adjusted whole-cluster residual permutation, and ordered
  endpoint-family gatekeeping. Workspace formatting, warning-denied all-target/all-feature Clippy,
  no-default compilation, all-feature doctests, and strict all-feature docs pass on the final
  production state.
- The full workspace integration/Nextest loop was not run because checkpoints 51/52 and the active
  instruction prohibit retrying the macOS binary-verification loop. No feature matrix, benchmark,
  fuzz, memory tool, packaging, dependency audit, push, publication, deployment, or history rewrite
  ran. The next production outcome remains multiclass CellViT geometry/marks with typed interchange.

## Multiclass CellViT categorical mixing checkpoint 114 — 2026-08-28

- Added `categorical_neighborhood_mixing` and a cache-addressed durable analysis node for a typed
  categorical MarkTable column with at least three declared classes. One exact physical-radius
  graph yields the complete directed class-by-class incidence matrix, observed fractions,
  fixed-count without-replacement random-label expectations/excesses, cross-class edge summary,
  zero-neighbor count, and per-source-class neighbor probabilities/entropy. Cells and edges remain
  descriptive structural units, never patient replicates.
- Exact class/row/status/provenance/frame/window/radius identity, graph/configuration digests,
  symmetric adjacency, finite fractions, and point/class/pair/retained-byte ceilings are enforced.
  The first durable test exposed a one-ULP ordinary-JSON entropy replay; the existing exact-f64 codec
  now preserves bit-identical miss/hit results without recomputation or tolerance relaxation.
- CSV and Parquet Pattern ingestion now retain the ordered categorical codebook alongside row codes
  under one backwards-compatible defaulted field. MarkTable binding rejects a codebook/declaration
  mismatch. The independent standard-library Python pair-loop oracle matches the 3x3 matrix,
  observed/null/excess fractions, cross-edge summary, and class entropies; its fixture regenerates
  byte-for-byte. New workflow tests pass 4/4, typed interchange 1/1, scalar MarkTable 11/11, legacy
  categorical 4/4, soft-simplex neighborhood 3/3, and Parquet boundaries 7/7.
- A read-only real-data check rehashed the admitted Mac-mini artifact at
  `/Volumes/1TB/marklab/runs/results-cellvit-2day-01/inputs/coordinate_scalar_cells.csv` as
  `86afdc0c343dc804258c57c1ecd99c7f9b544aa8363756c571570e6952e8858c` and its exact window as
  `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`. Production interchange
  retains 2,000 rows and exact counts: Connective 118, Dead 67, Inflammatory 365, Neoplastic 1,450.
  Durable real mixing remains unavailable because that derived CSV has zero stable `cell_id` rows;
  uncertainty-bearing real mixing remains unavailable because it has only winning-class confidence,
  not a complete class-probability simplex. No identities or probabilities were fabricated.

## Stable CellViT source identity and real durable mixing checkpoint 115 — 2026-08-28

- CSV and Parquet Pattern ingestion now retain one optional dense source `cell_id` column and
  materialize it through the existing typed `CellId` owner. Import rejects partial, malformed,
  duplicate, or non-increasing retained identities; older inputs remain valid through the defaulted
  absence state. The CPTAC adapter uses the exact slide ID plus zero-padded source row for both
  coordinate and vector lanes.
- The first real durable attempt found one exact threshold-binding defect: row 81 had been classified
  before its winning-class confidence was serialized and imported as `f32`. The adapter now derives
  the binary value from the exact imported representation. The accepted v3 export differs from the
  first stable-ID export only at that binary row and has zero binding mismatches; v1/v2 remain
  diagnostic inputs and are not promoted.
- The pinned Mac-mini CellViT environment revalidated all 366 slides, 178 patients, and 1,542,389
  rows into `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v3-inputs`. The exact 2,000-row
  coordinate input digest is `95b933c04a60abfef5cbf02784e7fd582dfdc6e3b129c0f6c84801e91c786709`;
  its unchanged window digest is `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`.
- Two fresh Rust test processes execute the real four-class, 50-micrometre workflow as miss then hit,
  return byte-identical typed output, and leave one durable execution. Focused interchange,
  categorical oracle/resource, scalar MarkTable, importer, adapter, formatting, syntax, LSP-outline,
  and whitespace checks pass. Full class-probability uncertainty remains unavailable because the
  admitted export has winning-class confidence only.

## Real CellViT pair curves and multiclass stabilization checkpoint 116 — 2026-08-28

- The accepted stable-ID CellViT table now drives the existing directed categorical mark-connection/
  cross-K and Epanechnikov cross-pair-correlation nodes for Neoplastic→Inflammatory organization at
  prespecified 25, 50, 100, and 200 micrometre radii. Cross-g uses a fixed 10-micrometre bandwidth;
  both workflows use 19 complete-row random-label permutations, seed 20260828, and explicit
  2,000-point, four-million-pair, 80-million-null-evaluation, and 128-MiB ceilings.
- Two fresh processes return miss then hit for both durable projects, retain one execution in each
  ledger, and reproduce exact typed-result digests. The admitted source/target counts are 1,450 and
  365, all four radii are present, and both bounded geometry plans contain directed pairs. This is a
  single-slide structural analysis under a fixed-count random-label null, not patient-population or
  molecular-class inference.
- The real typed hierarchy/MarkTable/provenance/window fixture is shared only by the three immediate
  CellViT integration callers. Existing independent hand/Python pair oracles, directional controls,
  one-short resource failures, and synthetic durable invalidation remain green.
- The four-workflow family since checkpoint 113 passed one major stabilization: workspace
  warning-denied all-target/all-feature Clippy in 25m26s, workspace no-default compilation in
  10.15s, all-feature doctests with zero failures, strict all-feature docs in 13.12s, formatting,
  and whitespace. The prohibited full-integration/Nextest loader loop was not run. The next
  production outcome is bounded durable project-DAG execution and cross-process whole-graph resume.

## Bounded parallel marked project DAG checkpoint 117 — 2026-08-28

- Added the first complete dependency-bearing project executor for the existing marked pre/post
  workflow. It plans the two root analyses from explicit per-root thread/memory declarations plus
  host node/thread/memory and total-row ceilings, runs both roots concurrently only when the plan
  admits them, and executes the typed comparison after exact dependency reconstruction.
- A roots-only target provides a real process boundary. The first fresh process records two root
  misses and ledgers `[1,1,0]`; the next restores both roots and records one comparison miss; the
  third returns three hits with ledgers fixed at `[1,1,1]`. Every node delegates objects, pending
  intent, ledger, head, recovery, cache keys, codec validation, and transaction semantics to its
  existing fixed durable-project shard.
- The comparison exactly equals the direct `compare_marked_prepost` owner. A one-node host budget
  deterministically serializes roots; Auto threads, a root exceeding host resources, and one-short
  total rows fail before execution. New behavior tests pass 4/4 and existing project workflows pass
  12/12. Targeted warning-denied Clippy, root no-default compilation, strict docs, LSP outline,
  formatting, and whitespace checks pass.
- No authorized CRC artifact currently contains paired pre/post or repeated timepoint CellViT
  Patterns, so real scientific pre/post execution is unavailable with that exact data blocker;
  synthetic fixtures are workflow/oracle evidence only. General heterogeneous DAG builders remain
  active. Production now returns to the next Bayesian likelihood/hierarchy with an immediate CRC
  caller.

## Durable patient multiclass CellViT Bayesian checkpoint 118 — 2026-08-28

- Added direct and durable patient-level Dirichlet-multinomial group regression through the pinned
  PyMC 6.3.0/Python 3.12 backend. Complete ordered class-count vectors retain zero counts, use one
  patient as the biological/likelihood unit, and expose exact class composition/difference,
  concentration, posterior-predictive, diagnostic, backend, environment, worker, request, and input
  identities under explicit patient/class/cell/iteration/output/time ceilings.
- The Mac-mini adapter revalidated the frozen 366-slide, 178-patient, 1,542,389-cell CPTAC CellViT
  source and wrote the fresh v4 bundle. Its molecular lane has 525 rows: every one of five hard
  annotation classes for 105 labeled patients (81 MSS, 24 MSI), including 31 exact zero counts. The
  input SHA-256 is `00f91c5a8084d3a1e9140beb9769dca2b74ecd4e08d9eef17c258f7504c5167e`.
- The real two-chain fit completed 4,000 retained draws with R-hat 1.00199, bulk/tail ESS
  1278.06/1708.53, minimum E-BFMI 1.00882, zero divergences, and zero tree-depth hits. Exploratory
  MSI-minus-MSS mean composition differences were -0.0843 Neoplastic, +0.0855 Inflammatory,
  -0.00784 Connective, +0.0116 Dead, and -0.00492 Epithelial. These are classifier-derived,
  unadjusted cohort associations, not causal, clinical, external-validation, or raw-probability
  evidence.
- A fresh process produced the durable miss; a second process with external backend execution
  disabled returned a byte-identical hit and left one ledger execution. The typed result SHA-256 is
  `8a00e2119d2e454f21be53fd666bf4200b60a78f38e57130ccf59ad0f931362c` and is stored with the v4
  bundle on the 1 TB drive. Synthetic behavior independently checks the exact observed
  patient-proportion arithmetic and known opposite Neoplastic/Inflammatory shifts. Broader
  two-backend agreement, prior sensitivity, and SBC remain the next promotion work rather than
  being inferred from this real fit.

## Multiclass Bayesian promotion and stabilization checkpoint 119 — 2026-08-28

- Added three bounded production commands around the checkpoint-118 likelihood. Agreement uses the
  identical typed request in pinned PyMC and NumPyro and checks every class's reference/comparison
  probability and group difference plus concentration. Sensitivity runs the fixed seven-scenario
  half/base/double one-at-a-time prior grid. SBC calibrates every free baseline logit, every group
  log-ratio effect, concentration, and every derived class-probability difference across 20–100
  prior-generative replicates with exact failure dispositions and work ceilings.
- Synthetic behavior passes complete three-class agreement, seven converged sensitivity fits, and
  20/20 full-vector SBC replicates with accepted rank-uniformity and 90% coverage for all eight
  calibrated quantities. The real 105-patient/five-class CPTAC input agrees across PyMC and NumPyro
  for all 16 declared comparisons with zero divergences/tree-depth hits in both fits. Its agreement
  result SHA-256 is `288bbeaf817e440caa492032af853222d7242cf66fbb7b9e0ee181a731ebaab4`.
- The real prior grid is diagnostically complete but not uniformly stable: only
  `logit_sd_lower` crosses the prespecified 0.75 posterior-SD threshold, at 0.9835; the other five
  nonbaseline scenarios remain below 0.415. The sensitivity result SHA-256 is
  `b4a2d308fa98ba1dc0a3278b0dbd705434e512bb4079ec5751f0f4f3da67f1ce`. Both results are sealed
  beside the base fit in the Mac-mini v4 bundle. This limitation is retained explicitly.
- The four-workflow family passes one scheduled stabilization: workspace formatting,
  warning-denied all-target/all-feature Clippy in 26m22s, no-default compilation in 21.06s,
  all-feature doctests with zero failures, and strict all-feature docs in 25.95s. The affected
  Bayesian package passes 47/47, and final direct/durable replay tests remain green. The prohibited
  Nextest/full-integration loader loop was not rerun. Production advances to the next immediate
  arbitrary-window point-process or hierarchy caller, not UI/server/client work.

## Representative sparse CellViT graph heat checkpoint 120 — 2026-08-28

- Added a user-visible sparse exact-radius graph heat workflow and durable project path. It uses a
  deterministic uniform-cell search to retain every radius edge, a binary combinatorial
  Laplacian, the conservative `[0,2*d_max]` spectral interval, and adaptive Chebyshev application
  without materializing a dense Laplacian or eigensystem. Isolated nodes retain their signals.
  Node, candidate, edge, matrix-vector, and conservative working-memory ceilings are explicit.
- Independent small controls agree with the existing dense eigensolver on a path and a 25-node grid
  spanning negative cell keys; candidate, edge, and working-byte one-short limits fail before an
  over-budget result. Durable output reuses the existing exact-f64 codec and proves miss, fresh
  process hit, byte identity, one ledger execution, and changed-input invalidation.
- The Mac-mini v5 adapter revalidated 366 slides, 178 patients, and 1,542,389 cells and emitted the
  exact 2,000-cell representative graph request with source CellIds and a hard Neoplastic indicator.
  Its SHA-256 is `49d5509ff4996b8e23baa8d0c8118fa579f43cd07c95ed9b301124ebc7e35dcf`.
  At 50 µm the graph has 24,755 edges and nine isolated nodes after 56,086 candidate checks. The
  selected order is 13, matrix-vector work is 669,630, accounted working memory is 2,268,208 bytes,
  measured wall time is 0.07s, and measured maximum RSS is 14,958,592 bytes.
- Direct and durable real outputs share SHA-256
  `769ece4d0b37345e053abff0c387da42feabaec47431db870dbee9c8fdb3f593` and the latter replays with
  one ledger record. This is a single-slide descriptive graph signal, not patient-population,
  topology, biological-calibration, or performance-generalization evidence. Arbitrary-window point
  processes, sparse wavelets/scattering/eigensolvers, and representative topology remain active.

## Representative CellViT witness persistence checkpoint 121 — 2026-08-28

- Added `marklab project witness-persistence` over the existing pinned GUDHI 3.13.0 worker. The
  durable node binds the exact source artifact, canonical typed request, GUDHI/Python versions,
  environment-lock and worker digests, executable/runtime identity, bounded process policy, result
  schema, and exact-f64 artifact codec. A fresh-process hit validates the typed result without
  starting GUDHI again.
- The CellViT adapter now emits one bounded witness request from the same exact 2,000 stable source
  CellIds and physical micrometre coordinates used by the representative graph lane: deterministic
  farthest-point selection, 64 landmarks, dimensions 0–2, `nu=0`, 200-micrometre maximum scale,
  field 2, 500,000-simplex ceiling, and 180-second process ceiling.
- The frozen Mac-mini sources revalidated 366 slides, 178 patients, and 1,542,389 cells into the v6
  bundle. The input SHA-256 is `deea7b94b38f2430e8004e9f26d607572669b93cc904dc6e9f4e844c31c40755`.
  Direct execution completed in 2.04 seconds at 40,288,256-byte maximum RSS, selected 64 landmarks
  with 118.624-micrometre coverage radius, and retained 388 simplices (64/150/174 by dimension).
- Direct, durable-miss, and backend-disabled durable-hit outputs are byte-identical with SHA-256
  `81eba331498cdebeb0cd460b6368db694e0260ada53a14df4a6a7c3acd346f40`; one ledger execution remains,
  and the typed result is sealed in the v6 bundle on the 1 TB drive. This is a bounded single-slide
  descriptive approximation, not patient replication, topology stability, molecular association,
  biological validation, or performance generalization. Those broader TOP-01/WS-63 requirements
  remain active.

## Exact-window CellViT Poisson likelihood checkpoint 122 — 2026-08-28

- Added direct and durable `arbitrary-window-ipp-likelihood` commands over the existing exact
  `ObservationWindow2D` owner. Events and positive weighted quadrature nodes must lie in the exact
  MultiPolygon, including hole and disconnected-component semantics; weights must conserve exact
  window area, and IDs, canonical input/window digests, coefficients, assumptions, finite-result
  policy, work, retained bytes, scheduler/runtime identity, and source artifacts are explicit.
- The independent constant-intensity oracle uses a four-square-micrometre two-component window with
  a one-square-micrometre hole. At intensity two it exactly returns event term `2*ln(2)`, integral
  eight, and likelihood `2*ln(2)-8`; a one-short work ceiling fails. Dedicated CLI parsing avoids
  adding another variant to the legacy Bayes parser after the expected green attempt exposed its
  pre-existing main-thread stack limit.
- The frozen Mac-mini adapter revalidated 366 slides, 178 patients, and 1,542,389 cells into the v7
  bundle. It retained 2,000 exact source-identified events and 552 positive clipped 64-by-64 cells
  whose weights sum to 787,061.271429 square micrometres for the exact 12-component patch union.
  The event/quadrature/window SHA-256 values are `965239e4c82c346e19fec097e26453812e7c8fe885cd495ecaf8ec8f3aef476c`,
  `42e759b5780340d38d1bcdf92cd7830e717df5a0c33cb62202233391666af146`, and
  `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`.
- At the constant-intensity MLE, the fixed likelihood has intercept -5.975159, integral 2,000,
  likelihood -13,950.318, declared work 2,552, and 232,296 retained bytes. Direct execution took
  0.05 seconds at 16,695,296-byte maximum RSS. Direct/miss/hit outputs are byte-identical with one
  ledger execution and SHA-256 `756c7b110fe6971e07e8bc94489a6d961c04f9667863af54f4cc905f4ff55c4a`;
  the result is sealed in the v7 bundle. This closes a fixed-likelihood arbitrary-window caller,
  not arbitrary-window Bayesian fitting, effect inference, interaction, or patient replication.

## Durable fitted exact-window CellViT IPP checkpoint 123 — 2026-08-28

- Added direct and durable `fit-arbitrary-window-ipp` through the unchanged pinned PyMC
  6.3.0/Python 3.12 rectangular IPP worker. A measure-preserving algebraic adapter maps each
  arbitrary positive quadrature weight to an equal-area synthetic backend cell by adding
  `ln(weight)` to its offset; the outer typed result restores exact node IDs, physical coordinates,
  weights, intensities per square micrometre, expected counts, and exact MultiPolygon identity.
- A two-component known positive-gradient control recovers a posterior coefficient above 0.5 with
  complete diagnostics. The durable control proves miss, backend-disabled fresh-process hit,
  byte-identical typed result, one ledger execution, and prior-change invalidation without changing
  or copying the existing static worker/environment.
- The real 2,000-cell/552-node v7 input completed 2,000 posterior draws in 2.26 seconds at
  335,675,392-byte maximum RSS. Diagnostics are R-hat 1.00293, bulk/tail ESS 1630.45/1272.84,
  minimum E-BFMI 1.21765, zero divergences, and zero tree-depth hits. The posterior horizontal
  coefficient is -0.4566 (95% interval -0.5210 to -0.3888); total expected count is 2000.97.
- Two independent backend executions differ only by floating reductions, with maximum absolute
  difference `1.93e-11` across 7,216 numeric fields; this is recorded rather than called byte
  deterministic. The durable miss/hit is byte-identical with one execution and canonical SHA-256
  `a65547c01d93e9ef51ee8821f9f42e0f61f8dd078a2b80f0c296f3a57ae89b2d`, sealed in the v7 bundle.
  The coefficient is an exploratory within-slide x-coordinate intensity gradient, not interaction,
  causality, molecular association, clinical evidence, or patient-population inference.

## Graph/topology/exact-window stabilization checkpoint 124 — 2026-08-28

- Stabilized checkpoints 120–123: sparse CellViT graph heat, witness persistence, exact-window fixed
  IPP likelihood, and pinned exact-window IPP fitting. Workspace formatting, all-target/all-feature
  warning-denied Clippy, no-default compilation, all-feature doctests, strict all-feature docs, and
  whitespace checks pass on the final production state.
- The prohibited full-workspace integration/Nextest loader loop was not run. No feature matrix,
  benchmark suite, fuzzing, memory tool, packaging, dependency audit, push, publication, deployment,
  or history rewrite ran. BAY-PP/WS-43 now advance to agreement, sensitivity, calibration, and
  spatial posterior-predictive promotion for the fitted arbitrary-window caller.

## Exact-window IPP agreement and prior sensitivity checkpoint 125 — 2026-08-28

- Added `arbitrary-window-ipp-agreement` using the identical transformed weighted likelihood in the
  existing PyMC worker and one narrow pinned NumPyro 0.21.0/JAX 0.11.1 worker. The NumPyro adapter
  imports and validates the existing PyMC request contract rather than duplicating it. Exact lock,
  PyMC-worker, NumPyro-worker, request, JAX/Python, sampling, diagnostics, and input identities are
  retained; disagreement remains a valid result.
- The known positive-gradient control passes both backends. On the real v7 input, PyMC versus
  NumPyro posterior means differ by 0.000352 for the intercept, 0.000793 for the coefficient, and
  0.631 expected cells; all are inside the prespecified 0.05/0.05/5.0 absolute gates with overlapping
  parameter intervals. Both fits are complete with zero divergences/tree-depth hits. The sealed
  agreement result SHA-256 is `dda4b0d95470b25af8f217839df55920a7ed53de6c1c84b619aa765d608b2b9e`.
- Added `arbitrary-window-ipp-sensitivity` for a fixed baseline plus half/double intercept- and
  coefficient-prior SD grid. All five real fits are complete. No scenario crosses the prespecified
  0.75 baseline-posterior-SD threshold; the maximum shift is 0.0603 for the half intercept-prior SD.
  The sealed result SHA-256 is `64e644a073123dc43ad36c0ae47a01a52841763718ac4570c450f53b1df7f2a7`.
- This promotes cross-backend and prior-scale evidence only. SBC, quadrature refinement, and spatial
  posterior-predictive checks remain active; the real negative x-gradient remains a single-slide
  descriptive association without population, causal, interaction, or clinical interpretation.

## Exact-window IPP diagnostic promotion checkpoint 126 — 2026-08-28

- Added bounded 48/64/80 exact-area quadrature sensitivity, prior-generative NumPyro SBC, and a
  PyMC spatial posterior-predictive command over exact event-to-node membership and a fixed
  physical-radius node graph. Every fit/scenario now retains backend, lock, worker, request, input,
  and sampling identity. The spatial caller reuses the existing PyMC validator, likelihood, and
  sampler; no second likelihood adapter or diagnostic registry was added.
- The Mac-mini adapter re-audited all 366 slides, 178 patients, and 1,542,389 cells and wrote the
  v10 bundle. Its 2,000 exact events map one-to-one to 2,000 membership rows over 381 occupied nodes;
  all memberships reference the 552-node baseline quadrature. Membership SHA-256 is
  `99f95d5850db48daa97441baae93c37dfd6bef7055f2b436b43898d35c624da6`.
- All 372/552/845-node quadrature fits complete; the largest shift is 0.00193 posterior SD and no
  resolution reaches the 0.75 materiality threshold. Under the deployed diffuse priors, SBC
  completes 16/20 replicates and truthfully returns `not_accepted`: four deterministic prior draws
  imply 21.5 million to 4.80 billion cells and exceed the ten-million count ceiling. The separately
  named physically scaled prior completes and accepts 20/20 replicates, with rank means
  0.389/0.512/0.414, 90% coverage 0.80/0.95/0.70, maximum R-hat 1.01666, and zero divergences.
- The real 100-micrometre PPC uses 3,233 physical neighbor pairs and 2,000 posterior-replicated
  patterns. The fit is complete, but only 4.05% of replicas meet the observed node-density variance
  and 2.0% meet the observed neighbor density contrast. This is evidence that the simple fixed
  x-gradient model under-replicates local spatial variation, not evidence of biological
  significance. The statistical unit remains one observed point pattern.
- Refactoring the shared sampler changed its exact worker digest, so the real durable fit and every
  affected diagnostic were refreshed once. The durable miss and backend-disabled fresh-process hit
  are byte-identical with one ledger execution. Final result SHA-256 values are
  `bc13a44e7ac7a17bc7ad3931712a16b9d19eabba4f318e7d0d8e58fae18b309b` fit,
  `0fd65d12afd1eb6d8649f0f10cf1d3378b4e65e94b62f8e95cc6fdd2a7e4262c` agreement,
  `e9ab431fd1f204e4e90e022730983fd4101302b2c528f4327cef125a480a71f8` prior sensitivity,
  `a5df627be5f735004d1130181eb73edf3e3866391a6dbf2a17c1ee542e4fa364` quadrature,
  `e1bb2af3d46be486ebcc28e1b1cbdb263e71c6ced9ab7eb1ee051ab1d11c264f` deployed-prior SBC,
  `74f06d3a5472f0b5aa95b197d9a2c3f66a85ea8eb21a8a28c92a3450fa67725a` physical-prior SBC,
  and `ead46ba7f9f05aa7b0aa654e41e774853a5e14adbce3f023380ad0e73e963d42` spatial PPC; the complete
  hashes and successful `shasum -c` manifest are sealed in the v10 results directory.
- Focused analytic/integration tests, all seven adapter tests, targeted warning-denied Clippy,
  package no-default compilation, formatting, and whitespace checks pass. The prohibited
  full-workspace integration/Nextest loader loop and unrelated broad gates were not run. BAY-PP and
  WS-43 remain active for a scientifically justified latent spatial field, multitype/marked models,
  and independent patient patterns; UI/server/client work remains deferred.

## Durable exact-window CellViT latent-field checkpoint 127 — 2026-08-28

- Added direct and durable `fit-arbitrary-window-lgcp` execution through the existing pinned PyMC
  6.3.0 dense gridded-LGCP worker. The narrow adapter preserves exact MultiPolygon identity,
  positive area-partition weights, exact event-to-node membership, and physical node coordinates;
  it replaces only the source worker's synthetic grid covariance with a fixed physical Matérn-3/2
  covariance. The durable node binds all four source artifacts, both worker identities, the lock,
  request, fixed kernel, sampling controls, and resource policy to the existing scheduler, ledger,
  artifact store, and exact-f64 codec.
- A four-node exact-window control is algebraically identical to the existing rectangular LGCP and
  agrees within `1e-10` for the intercept, coefficient, and every latent-field mean. Its spatial
  check independently recovers observed node-density variance 28.75 and four-pair mean density
  contrast 7.0. Cross-process durable miss/hit output is byte-identical with one ledger execution
  and no second backend start.
- The Mac-mini adapter re-audited 366 slides, 178 patients, and 1,542,389 cells into the v11 bundle.
  The admitted single-slide field has 2,000 exact events, 2,000 exact membership rows, and 32
  positive clipped nodes over 787,061.271429 square micrometres. The final 4,000-draw fit is
  complete: R-hat 1.00492, bulk/tail ESS 1783.98/2034.15, minimum E-BFMI 0.94255, zero divergences,
  and zero tree-depth hits. Its exploratory horizontal coefficient is -0.5597 with interval
  [-1.1049, 0.0173]; the result SHA-256 is
  `8ff67152ce931e51b11399b73344c178d56b1c61c0dd58772fc414644514142e`.
- The 32-replicate physical spatial PPC is negative: no replica reaches the observed node-density
  variance or 600-micrometre neighbor contrast. A fixed five-scenario half/base/double amplitude
  and length-scale grid completes in 57.44 seconds at 349,552,640-byte maximum RSS; no scenario
  crosses the prespecified 0.75 standardized-shift threshold, with maximum 0.694 at the doubled
  length scale. Its sealed SHA-256 is
  `da284929a3a1e6cd4b0f83b27a570754a257f621b5fafbd40e600937b92ad2fc`.
- This is a coarse, fixed-kernel, single-pattern latent-intensity model. It is not evidence of point
  attraction, a patient-population effect, biological significance, or adequate residual spatial
  fit. Focused direct/durable/oracle tests, the seven adapter tests, Python syntax checks, targeted
  warning-denied Clippy, package no-default compilation, affected-file formatting, and whitespace
  checks pass. BAY-PP/WS-43 remain active for quadrature/component sensitivity, inferred field
  hyperparameters with calibration, multitype/marked likelihoods, and independent patient patterns.

## Exact-window latent-field promotion checkpoint 128 — 2026-08-28

- Added independent NumPyro 0.21.0/JAX 0.11.1 agreement and prior-generative SBC around the exact
  physical covariance without copying either sampler. Both narrow adapters validate and call the
  existing PyMC exact-window contract plus the existing NumPyro gridded-LGCP fit/SBC owners, while
  retaining outer/source workers, lock, JAX, request, input, covariance, sampling, calibration, and
  resource identities.
- The real PyMC/NumPyro fit agrees under the existing Monte Carlo-aware rules. Intercept and
  coefficient mean differences are 0.00215 and 0.00492; every one of 32 latent-effect and
  expected-count intervals overlaps. Maximum standardized field differences are 0.744 latent and
  1.580 expected count, both below 5.0; both fits are complete with zero divergences/depth hits.
  Result SHA-256 is `8f80eecad54f21b964317029bd846d4a597f658854657f3521a2ec25d324f635`.
- The deployed prior completes and accepts 20/20 physical-field SBC replicates. Intercept,
  coefficient, and prespecified physical-node rank-uniformity p-values are 0.163/0.534/0.834,
  normalized mean ranks 0.496/0.497/0.587, and 90% coverage 0.90/0.80/0.90. No replicate is hidden or
  replaced; SHA-256 is `9670d00a8e003de46d5ceeafda09cca6dc7b4fd219f652a80b8d1ad15274fa62`.
- The Mac-mini adapter re-audited the frozen 366-slide, 178-patient, 1,542,389-cell source into the
  v12 bundle and emitted exact 4/6/8 bounding-grid partitions with 11/22/32 positive clipped nodes.
  All three 4,000-draw fits complete. Relative to the deployed 32-node result, the maximum global
  shift is 0.512 posterior SD and total expected-count differences are below 0.075%, so neither
  coarser partition reaches the prespecified 0.75 materiality threshold. The quadrature result
  SHA-256 is `c32b4903302c944a674dc4b1a2bbbf60a64a8c618bd2a7f11249b105c9b8e487`.
- Spatial PPC tails vary with aggregation: the 11-node check has 0.25 upper tails, whereas the
  deployed 32-node check remains zero for both summaries. Thus global posterior resolution
  stability does not rescue spatial adequacy or make cross-resolution node-density summaries
  interchangeable. The v12 manifest verifies all five base/promotion results by SHA-256.
- This four-workflow family passes one scheduled stabilization: workspace formatting,
  all-target/all-feature warning-denied Clippy, no-default compilation, all-feature doctests, strict
  docs, and whitespace checks. The prohibited full-workspace integration/Nextest loader loop was
  not rerun. Exact-window fixed-kernel LGCP no longer lacks agreement, SBC, prior/kernel sensitivity,
  quadrature sensitivity, physical PPC, or durable replay. BAY-PP/WS-43 advance to an independent-
  patient replicated-pattern or immediate multitype/marked CRC caller.

## Durable replicated-patient CellViT field checkpoint 129 — 2026-08-28

- Added direct and durable `fit-replicated-arbitrary-window-lgcp` execution for exact slide patterns
  nested inside independent patients. Separate window/event identities and physical nodes feed a
  fixed Matérn field per slide, patient and slide random intercepts, and MSI/MSS plus within-slide x
  effects. Slide effects sum to zero within patient and fields within slide; patient effects remain
  unconstrained zero-mean draws so patients govern group uncertainty.
- A known-shift control uses eight patients, two patterns each, and 64 nodes. It recovers the
  positive group shift with complete diagnostics. Its durable path proves miss, fresh-process
  backend-disabled hit, byte-identical exact-f64 output, one ledger execution, and typed replay.
  Prior finiteness is checked with 500 deterministic prior draws.
- The Mac-mini adapter selected four provenance-sorted repeated-slide patients per MSI/MSS group and
  two slides each from the frozen 366-slide, 178-patient, 1,542,389-cell source. The v14 input has 16
  exact slide patterns, 222 positive clipped nodes, and 69,377 cells; SHA-256 is
  `4c19d880519901524db244c3891eb165222df3c169a4278090caa983d5bc0ee7`.
- The first unconstrained diagnostic had 4 divergences and 3,731 depth hits. A group-wise
  patient-centering attempt converged but was rejected because it conditioned away between-patient
  group uncertainty. Centering only slide effects within patient and fields within slide produced
  the retained population-valid fit; neither rejected artifact is final evidence.
- The final durable miss completed in 171.81 seconds at 453,492,736-byte maximum RSS. R-hat is
  1.00772, bulk/tail ESS 999.07/1401.69, E-BFMI 0.95955, and divergences/depth hits are zero. The
  exploratory MSI-minus-MSS log-intensity effect is 0.1869 with interval [-0.5178, 0.8849]; patient
  SD is 0.5503 and slide SD 0.7633. This is a null-compatible eight-patient subset result, not
  molecular, causal, clinical, or transportability evidence.
- The backend-disabled hit is byte-identical with one ledger execution. Final SHA-256 is
  `e088d056e48cd28aade956068000f74d9cddd761386c7efb28b7677082d15161` and the v14 final manifest
  verifies it. Focused tests, all eight adapter tests, Python syntax, targeted Clippy, no-default
  compilation, formatting, and whitespace checks pass. Agreement, sensitivity, SBC, and
  replicated-pattern PPC remain the next promotion work; broad gates were not repeated after 128.

## Replicated-patient CellViT field promotion checkpoint 130 — 2026-08-29

- The durable fit result is now typed version 2 with every physical node's latent and expected-count
  posterior plus deterministic per-slide total-count and node-variance predictive checks. The real
  miss completed in 168.43 seconds at 458,604,544-byte RSS; a fresh backend-disabled process returned
  a byte-identical hit with one ledger row. SHA-256 is
  `f9bc6c51ed0cfd5b18558855dc5115853448580ab14cd6347f68949fdbcbe08f`.
- The posterior and complete diagnostics are unchanged: the MSI-minus-MSS log-intensity effect is
  0.18694 with interval [-0.51781, 0.88489]. All 16 deployed-pattern total and coarse node-variance
  checks are non-rejecting; minimum two-sided tails are 0.9365 and 0.0555, respectively.
- Independent NumPyro agreement required an explicit sampler-specific depth capacity. Depth 10 and
  12 left 1,211 and 3 ceiling hits and remain rejected diagnostics. Depth 13 completed in 279.10
  seconds with R-hat 1.00386, bulk/tail ESS 1434.62/1966.82, E-BFMI 0.92293, and zero divergences/
  depth hits. Every global, patient, slide, latent-node, and expected-count interval overlaps; the
  largest standardized differences are 0.124 globally and 1.481 over nodes. SHA-256 is
  `2f5051523b5b21a06cefd1c978499fc8e61821e27190864b8542b06277fcfbe9`.
- The fixed nine-scenario real grid completed in 1,841.59 seconds. Patient/slide prior-scale changes
  do not cross 0.75 SD, but field amplitude/length changes materially alter slide effects, up to
  2.079 SD. Three scenarios have 4, 250, and 6 tree-depth hits; halved amplitude and doubled length
  produce minimum node-variance tails of zero. The fixed spatial decomposition is therefore unstable,
  even though the group effect shifts by less than 0.08 SD. SHA-256 is
  `b9bb082f9b8c1d7fc2e6ae9505e70a1fc2bcf87158c719054aae40563af4b8ce`.
- Deployed-prior real-geometry SBC is rejected: 14/20 draws exceed the 69,377-event ceiling and two
  admitted fits saturate tree depth. A separately named count-scale prior passes 20/20 in 252.49
  seconds; rank-uniformity p-values are 0.163–0.834 and 90% coverage is 0.80–1.00. Its SHA-256 is
  `481f9969bf5d684b3e199cc58b6161f7162ef5241d05a134ac3c69b00b53c16a`.
- The hash-verified bundle, including every rejected diagnostic and the durable project, is on the
  Mac mini 1 TB drive at `results-cellvit-categorical-v15-lgcp-promotion`. Five focused final-state
  integrations, the builder, Python syntax, affected and workspace all-feature Clippy, workspace
  formatting/no-default/doctests/strict docs, and whitespace checks pass. The prohibited full
  integration/Nextest loop was not run. This is null and unstable evidence, not a biological,
  interaction-process, causal, clinical, significance, or transportability claim.

## Inferred shared-kernel replicated CellViT field checkpoint 131 — 2026-08-29

- Added direct and durable `fit-replicated-arbitrary-window-lgcp-inferred-kernel`. It reuses the
  exact DEC-0312 patient/pattern/window request and pinned PyMC environment but infers one positive
  shared Matérn amplitude and micrometre length scale. Sixteen symbolic per-slide block Choleskies
  replace the fixed covariance under exact 222-node, 888,000 draw-node, 45,750 pattern-cube, and
  1,200-second ceilings.
- The known-shift control recovers its patient-group effect with finite inferred scales, all node
  posteriors, 16 predictive rows, and complete diagnostics. Its durable test proves miss, fresh-
  process backend-disabled hit, byte identity, one ledger row, and exact-f64 replay.
- The first real fit at depth 10 completed in 527.65 seconds but had 377 tree-depth hits and is
  rejected. With only the explicit capacity changed to 13, the final real fit completed in 582.67
  seconds at 475,627,520-byte RSS with R-hat 1.00453, bulk/tail ESS 514.84/830.86, E-BFMI 0.65048,
  and zero divergences/depth hits.
- Shared amplitude is 0.6192 with interval [0.5357, 0.7187]; length scale is 335.3 micrometres with
  interval [251.5, 434.9]. The MSI-minus-MSS effect remains null-compatible at 0.1972 with interval
  [-0.5878, 0.9295]. Minimum total-count/node-variance predictive tails are 0.95/0.7395. This
  addresses the fixed-grid instability without claiming attraction or biological significance.
- The backend-disabled hit is byte-identical with one ledger row. SHA-256 is
  `4ddb116a550c61dabc76c6eef10a25c646b9445748692d076b83921ff39a0b8f`; the v16 Mac-mini 1 TB
  bundle verifies the input, final result, and rejected depth-10 diagnostic. Direct/durable tests,
  Python syntax, affected Rustfmt, warning-denied Clippy, package no-default compilation, and
  whitespace checks pass. Broad gates were not repeated after checkpoint 130.

## Inferred shared-kernel agreement/calibration checkpoint 132 — 2026-08-29

- Added `replicated-arbitrary-window-lgcp-inferred-kernel-agreement` with an independent pinned
  NumPyro dynamic-kernel implementation. The real 16-slide result is complete in both backends:
  NumPyro R-hat 1.00873, bulk/tail ESS 555.24/798.35, E-BFMI 0.62361, and zero divergences/depth
  hits. All seven global, eight patient, 16 slide, 222 latent-node, and 222 expected-count comparisons
  pass; SHA-256 is `7e1336748f4cf32b629429865e0869d3213c5fd4a8cc0634ae8f2799150050e0`.
- Added `replicated-arbitrary-window-lgcp-inferred-kernel-sbc`. Its 20-replicate synthetic oracle
  passes ranks and 90% coverage for the group effect, patient/slide scales, inferred amplitude,
  physical length, and one physical latent node. The worker retains every failed replicate and exact
  backend/environment/worker/source-request identity under event/node/iteration/kernel/time/output
  ceilings.
- Full admitted 222-node real-geometry count-scale SBC is not fully calibrated. The 4x1,000-draw
  run completes 19/20 because replicate 18 has R-hat 1.01048; SHA-256 is
  `ee77fe54cdce6a935471cc317b925fbd0d8b8599d1c4cd619ddb634b2440bbf0`. The only declared capacity
  retry, 4x1,500 draws with unchanged seed/priors/geometry/thresholds, also completes 19/20 because
  replicate 15 has one divergence; SHA-256 is
  `c366c0b8a09053383296b4f5ce86bcb07db51cfd40a02cd9aa2a68b509ea8b28`. All completed-replicate
  aggregate rank p-values and coverage bounds pass, but the strict no-failure rule correctly keeps
  both results nonconverged.
- The hash-verified v17 Mac-mini 1 TB bundle is
  `results-cellvit-categorical-v17-lgcp-inferred-promotion`; it retains the canonical durable fit,
  real agreement, both negative SBC attempts, the rejected depth-10 fit, unchanged input, and claim
  limitations. Real SBC elapsed/maximum RSS were 814.20 s/6,024,445,952 bytes and
  1,063.25 s/6,345,883,648 bytes. Focused agreement/SBC integrations, Python syntax, affected
  Rustfmt, warning-denied affected Clippy, package no-default compilation, bundle checksums, and
  whitespace checks pass. Broad gates were not repeated after checkpoint 130.

## Conditional hard-multitype CellViT mark checkpoint 133 — 2026-08-29

- Added direct and durable `fit-conditional-multitype-mark` through pinned PyMC 6.3.0. It consumes
  the existing typed CellViT CSV interchange, fixes the exact physical-radius location graph, and
  fits all 3–8 hard types jointly through symmetric single-site categorical conditionals. Reference
  intercept and reference/reference potential are exactly zero; reported pair affinities are
  gauge-invariant cross-minus-average-homotypic contrasts.
- The three-class separated-cluster oracle recovers negative affinity intervals and improves the
  composite conditional score over intercept-only independent labeling. The durable test proves
  miss, fresh-process backend-disabled hit, byte identity, and one ledger row. The result labels its
  type-count/edge check as a one-step conditional expectation, not a joint posterior simulation.
- The unchanged admitted 2,000-cell CPTAC table retains Neoplastic 1,450, Inflammatory 365,
  Connective 118, and Dead 67 on 24,755 exact 50-micrometre edges. Its real fit completes in 30.00
  seconds at 879,837,184-byte maximum RSS with R-hat 1.00110, bulk/tail ESS 1,601.24/1,896.05,
  E-BFMI 0.96426, and zero divergences/depth hits. All six affinity intervals are negative; the
  conditional score improvement is 777.26 and the one-step expected same-type edge interval
  [20,586.6, 21,215.4] contains 20,898 observed.
- A fresh backend-disabled process returns the byte-identical hit with one ledger row. Result SHA-256
  is `03ddbacba4b729ac0814079298067cf3499d9f9427028a27e1197d68a0c5a371`; the final v19 Mac-mini
  1 TB bundle `results-cellvit-categorical-v19-conditional-multitype` verifies its source, result,
  durable control/ledger, and limitations. This is single-slide fixed-location descriptive evidence,
  not a normalized joint Gibbs model, exact-count random-label test, patient effect, location-process
  attraction, biological, causal, clinical, significance, or transportability claim.
- Focused direct/durable integrations, Python syntax, affected Rustfmt, warning-denied affected
  Clippy, package no-default compilation, bundle checksums, and whitespace checks pass. Broad gates
  were not repeated after checkpoint 130.

## Conditional hard-multitype promotion checkpoint 134 — 2026-08-29

- Added `conditional-multitype-mark-agreement` with an independent pinned NumPyro implementation.
  The real PyMC and NumPyro fits are both complete with zero divergences/depth hits. All four
  intercepts, ten symmetric potentials, six invariant affinities, four expected type counts,
  composite score, and expected same-type edge count pass. Maximum standardized differences are
  0.186/0.038/0.033 for intercepts/potentials/affinities and 1.155 for the score. Real agreement runs
  in 37.30 seconds at 1,308,311,552-byte maximum RSS; SHA-256 is
  `50299b063e86bfba900f4587100544a7adf550855e0c5a6372b446701f41bf8f`.
- Added a fixed five-fit half/base/double PyMC prior grid. The synthetic separated-class control
  correctly reports material interaction-prior sensitivity rather than promising universal
  stability. All five real fits complete; the largest affinity shift is 0.647 baseline SD for the
  half interaction prior and the other scenario maxima are at most 0.190 SD, below the prespecified
  0.75 threshold. The real grid runs in 106.77 seconds at 1,139,654,656-byte maximum RSS; SHA-256 is
  `f923906bade54e9ac62b309bbecb1fbf341f07c8d3d5cccb0368e9bc3e31fe6c`.
- The hash-verified v20 Mac-mini 1 TB bundle is
  `results-cellvit-categorical-v20-conditional-multitype-promotion`; it contains the unchanged
  source, base fit, agreement, sensitivity, and exact limitations. Four focused direct/durable/
  agreement/sensitivity integrations, both worker syntax checks, affected Rustfmt, warning-denied
  affected Clippy, package no-default compilation, checksums, and whitespace checks pass. Exact
  finite-state calibration remains active; broad gates were not repeated before that fifth workflow.

## Exact finite-state conditional multitype calibration checkpoint 135 — 2026-08-29

- Added `conditional-multitype-mark-sbc`. It enumerates all joint label states on a bounded graph,
  samples exact normalized symmetric Gibbs labels from prior-drawn truths, and refits the same
  conditional composite pseudoposterior. It retains every disposition and calibrates all free
  intercepts/potentials plus gauge-invariant affinities under explicit state, enumeration-work,
  enumeration-byte, iteration, result, and process ceilings.
- The analytic oracle has nine sites, three types, eight edges, exactly 19,683 states, and
  zero-parameter log normalizer `9*ln(3)`. All 20 replicates complete. Across ten calibrated
  quantities, rank-uniformity p-values are 0.0487–0.991 and 90% coverage is 0.75–1.00, clearing the
  prespecified 0.001 and 0.65–1.00 bounds. The final run completes in 19.94 seconds at
  1,473,626,112-byte maximum RSS; SHA-256 is
  `64e933ef674901e2b1ac2cda73bb0ad322be14a93c75fddfe54cdc41ac5edeea`.
- The hash-verified v21 Mac-mini 1 TB bundle is
  `results-cellvit-categorical-v21-conditional-multitype-calibration`. This small-graph oracle does
  not replace real CellViT evidence, claim large-graph calibration, or normalize the pseudolikelihood.
  Its focused integration and worker syntax pass. This is the fifth cohesive workflow since
  checkpoint 130, so one major stabilization follows; the prohibited Nextest/full-integration loader
  loop remains excluded.
- The five-workflow major stabilization passes workspace formatting, warning-denied all-target/
  all-feature Clippy, workspace no-default compilation, all-feature doctests, and strict all-feature
  workspace docs. The documented macOS Nextest/full-integration loader loop was not retried.

## Replicated-patient conditional CellViT marks checkpoint 136 — 2026-08-29

- Added direct and durable `fit-replicated-conditional-multitype-mark` through pinned PyMC 6.3.0.
  The model fixes separate exact slide radius graphs, keeps patient as the population unit, nests
  two slides per patient, and separates composition-intercept from spatial pair-potential baseline,
  MSI-minus-MSS, patient, and slide variation under exact reference gauges.
- The known-shift control uses eight patients, 16 patterns, and 720 cells. An independent exact-grid
  oracle gives 1,936 total radius edges and 114/30 same-type edges per separated/mixed pattern; all
  three group affinity intervals recover the positive shift. Its durable path proves miss, fresh-
  process backend-disabled hit, byte identity, and one ledger row.
- The Mac-mini adapter revalidated all 366 slides, 178 patients, and 1,542,389 cells. It reused the
  exact replicated-LGCP patient/slide selection, filtered to the common Neoplastic, Inflammatory,
  and Connective vocabulary, and sampled only by stable source CellId. A 30,130-cell attempt exceeded
  its 1,200-second limit at 1,208.22 seconds and 898,449,408-byte maximum RSS. The retained bounded
  input has eight patients, 16 slides, 8,192 cells, and SHA-256
  `ebecd5a5bf47261bdaeb965255b5b5bfd3f74f6bebf01ca5c5e8447856a7e9c6`.
- The target-acceptance 0.95 fit completed in 548.66 seconds but retained 76 divergences and remains
  nonconverged. The only capacity retry used unchanged data, priors, seed, estimand, and diagnostic
  thresholds with target acceptance 0.99, 2,000 warmup iterations, and depth capacity 13. It is
  complete: R-hat 1.00729, bulk/tail ESS 630.90/858.52, E-BFMI 0.69379, and zero divergences/depth
  hits. The first completed run took 1,201.20 seconds at 731,693,056-byte maximum RSS. After
  affected-file formatting changed the exact native runtime digest, the versioned final durable
  execution reproduced it byte-for-byte in 1,197.25 seconds at 677,183,488-byte maximum RSS.
- MSI-minus-MSS affinity intervals are null-compatible: Neoplastic–Connective -0.0259
  [-0.1774, 0.1195], Neoplastic–Inflammatory -0.00934 [-0.1640, 0.1460], and Connective–Inflammatory
  0.00544 [-0.1624, 0.1654]. This is not equivalence, absence-of-biology, joint-Gibbs,
  location-attraction, causal, clinical, external-validation, or transportability evidence.
- A fresh backend-disabled process returns the byte-identical hit with one ledger row. Result
  SHA-256 is `15a3b52b498280f0baf80e0f53b2fa9b33b059936ceffec50ceca77237bcc8e4`;
  the hash-verified Mac-mini 1 TB bundle is
  `results-cellvit-categorical-v25-replicated-conditional-multitype`. Focused direct/durable tests,
  the nine-test adapter suite, Python syntax, affected warning-denied Clippy, package no-default
  compilation, affected Rustfmt, and whitespace checks pass. Broad gates were not repeated after
  checkpoint 135.

## Replicated conditional-mark promotion checkpoint 137 — 2026-08-29

- Added `replicated-conditional-multitype-mark-agreement`. It validates the exact typed PyMC result
  against a freshly prepared source request and runs only the independent NumPyro implementation.
  The real eight-patient/16-slide result is complete at 4 chains, 2,000 warmup, 1,000 retained draws,
  target acceptance 0.99, and depth 13. All 101 baseline/group/patient/scale/score/pattern
  comparisons pass; NumPyro R-hat is 1.00454, bulk/tail ESS 588.24/866.40, E-BFMI 0.69987, with
  zero divergences/depth hits. Result SHA-256 is
  `1aaa8fa967347b03d2d60b8627c7e26adbc4d3df5b8a4c87274f6a9a30363a47`.
- Added `replicated-conditional-multitype-mark-sensitivity`. Four fixed NumPyro fits vary only the
  patient or pattern hierarchy scale prior by 0.5x/2x around the exact PyMC baseline. All complete
  in one four-process wave. No group, patient, or hierarchy posterior shift reaches the declared
  0.75-SD threshold; the largest observed hierarchy shift is 0.44044 SD and the largest group shift
  is 0.01588 SD. Result SHA-256 is
  `41a909c6ea98397b892dc4f33b60f26b8732296188440913f6ce1a82ffb82a0b`.
- Added `replicated-conditional-multitype-mark-sbc`. For each prior draw it enumerates all 729 joint
  states separately on every six-site pattern, samples exact normalized Gibbs labels through the
  full patient/pattern hierarchy, and refits the production NumPyro composite model. It calibrates
  six invariant baseline/group affinities and four hierarchy scales with exact dispositions and
  state/work/byte/iteration/output/time ceilings. The first 2x1,000-draw run completed 15/20; five
  retained failures were exclusively R-hat 1.01025–1.01312 with adequate ESS/E-BFMI and zero
  divergences/depth hits. The single unchanged-model/seed/threshold retry at 2x1,500 draws completes
  20/20 and clears the declared rank/coverage bounds. The analytic zero-parameter normalizer is
  `6*ln(3)` per pattern. This is bounded small-pattern calibration, not normalized or 8,192-cell
  joint-likelihood evidence.
- The final exact calibration integration passes in 74.73 seconds. The related fit, durable replay,
  agreement, and sensitivity integrations all pass (15.49, 26.07, 45.27, and 45.00 seconds).
  Focused warning-denied Clippy, package no-default compilation/docs, Python syntax, affected-file
  Rustfmt, and whitespace checks pass.
- This four-workflow family boundary passes workspace formatting, warning-denied all-target/
  all-feature Clippy, workspace no-default compilation, all-feature doctests, strict all-feature
  workspace docs, and final whitespace checks. The documented macOS Nextest/full-integration loader
  loop was not retried. No feature matrix, benchmark, fuzzing, memory tool, packaging, dependency
  audit, push, publication, deployment, or history rewrite ran.

## Replicated exact-window multitype LGCP checkpoint 138 — 2026-08-29

- Added direct and durable `fit-replicated-arbitrary-window-multitype-lgcp` through pinned PyMC
  6.3.0. Every admitted hard type has one count on every positive exact-window node. The model keeps
  patient as the population unit, slides nested inside patient, and separate type-specific group,
  patient, slide, x-covariate, and fixed-Matérn field terms. It reports every type posterior, all
  pairwise group-effect differences, hierarchy/node posteriors, and slide/type count and node-
  variance predictive checks. It explicitly identifies independent Cox fields rather than Gibbs
  interaction.
- The synthetic eight-patient/16-slide/three-type oracle recovers an A-only comparison-group shift
  and a positive A-minus-B differential. The same integration rejects an incomplete node-type table
  and a one-short 192-row ceiling before backend execution. Its direct 2/2 and durable 1/1 tests pass
  in 13.28 and 22.31 seconds; the durable test proves miss, fresh backend-disabled hit, byte identity,
  and one ledger row.
- The Mac-mini adapter re-audited 366 slides, 178 patients, and 1,542,389 cells in 103.36 seconds at
  643,956,736-byte maximum RSS. The admitted table has eight patients, 16 exact slide windows, 222
  positive nodes, 666 node-type rows, and 65,570 cells in the complete Neoplastic/Inflammatory/
  Connective filtered event partition. Input SHA-256 is
  `c862ce9790274f1f099f040716de864b5185cc3084004ccef7693376a95f791f`.
- The identity-final real durable miss completed in 256.49 seconds at 641,417,216-byte maximum RSS.
  It is complete with R-hat 1.00530, bulk/tail ESS 1172.17/1634.16, E-BFMI 0.95063, zero divergences,
  and zero depth hits. Neoplastic MSI-minus-MSS is -0.2319 [-1.0402, 0.5617], Connective 0.00867
  [-0.6325, 0.6324], and Inflammatory 0.2367 [-0.7657, 1.2031]; every pairwise group-effect
  difference also spans zero. This is null-compatible, not equivalence or absence-of-biology
  evidence. Result SHA-256 is
  `84beaf85b062cbe3641ae4cbc4a50037954684408dafcb56cc96985e4d9255b1`.
- A fresh backend-disabled process returns the byte-identical result with one execution row. The
  hash-verified Mac-mini 1 TB bundle is
  `results-cellvit-categorical-v28-replicated-multitype-lgcp`. The earlier v27 bundle is retained as
  pre-final-validator identity evidence; its scientific posterior and diagnostic subtrees are
  byte-equivalent to v28 and it is not canonical.
- The adapter suite passes 10/10; both changed Python files pass syntax compilation. Focused
  warning-denied Clippy, package no-default compilation, affected-file Rustfmt, and whitespace checks
  pass. Broad workspace gates were not repeated immediately after checkpoint 137; no Nextest/full
  integration loop, feature matrix, benchmark, fuzzing, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.

## Replicated multitype LGCP promotion checkpoint 139 — 2026-08-29

- Added exact-baseline `replicated-arbitrary-window-multitype-lgcp-sensitivity`. Eight deterministic
  NumPyro scenarios vary patient SD, slide SD, fixed field amplitude, and fixed field length by
  0.5x/2x in bounded four-process waves while retaining every full typed fit. The small planted-
  shift oracle truthfully finishes seven scenarios and retains one doubled-length divergence after
  its single 2x2,000-draw capacity increase.
- The real 8-patient/16-slide/666-row grid completes all eight scenarios in 407.87 seconds at
  1,204,912,128-byte maximum RSS, with zero divergences and depth hits throughout. Type-group and
  pairwise group-difference shifts remain below 0.1231/0.1264 baseline SD. Field scenarios move
  slide effects and local expected counts by up to 3.4168/8.5574 SD, so the population contrast is
  stable under this grid but local reconstruction is materially fixed-kernel-sensitive. Sensitivity
  SHA-256 is `38586b490710c2f7529093f51fe9fef7928cc231a6c7820e05870588b455ccdc`.
- Added `replicated-arbitrary-window-multitype-lgcp-prior-calibration` through NumPy 2.4.6 in the
  existing pinned Python lock. Its independent generator compares five Gaussian/half-normal prior
  families with analytic moments, centered fields with exact `HLLᵀH` covariance, and conditional
  Poisson residuals with zero-mean/unit-second-moment oracles under exact simulation-work and a
  conservative Python/NumPy working-byte ceiling. The behavior test caught and corrected a type/node
  covariance-axis interleave before passing.
- The 4,096-draw identity-final real calibration completes in 1.47 seconds at 393,920,512-byte maximum RSS under a
  673,972,224-byte estimate and 1-GiB ceiling. Every prior-moment error is below 0.016, field
  covariance RMSE is 0.00752 amplitude squared, and 2,727,936 Poisson residuals have mean -0.00119
  and second moment 0.99993. Calibration SHA-256 is
  `956211a290cf4a8d781ef2be4ac61dc59d0cba4085c503e885f90e0dc14b3e6c`.
- Together with checkpoint 138 and DEC-0324 agreement across all 1,422 quantities, this closes the
  immediate fixed-kernel promotion ladder without claiming posterior SBC or invariant local fields.
  The hash-verified 1-TB bundle is
  `results-cellvit-categorical-v30-replicated-multitype-lgcp-promotion`. The v29 pre-final-identity
  bundle is retained as noncanonical evidence.
- Focused agreement and sensitivity integrations pass in 22.29 and 51.42 seconds; the identity-final
  NumPy calibration integration passes in 3.37 seconds. Python syntax, focused warning-denied Clippy, package no-default CLI compilation, remote
  bundle verification, workspace formatting, all-target/all-feature warning-denied Clippy,
  workspace no-default compilation, all-feature doctests, strict docs, and whitespace checks pass.
  The documented Nextest/full-integration loader loop was not run. No feature matrix, benchmark,
  fuzzing, memory tool, packaging, dependency audit, push, publication, deployment, or history
  rewrite ran.

## Replicated multitype inferred-kernel checkpoint 140 — 2026-08-29

- Added direct and durable `fit-replicated-arbitrary-window-multitype-lgcp-inferred-kernel` through
  pinned PyMC 6.3.0. It reuses the exact checkpoint-138 patient/type/window request, type-specific
  hierarchy and independent latent fields, but infers one shared positive Matérn amplitude and
  physical length inside the model. Dynamic Cholesky factors are built separately for each slide;
  exact source, backend, source-worker, inferred-worker, request, kernel-prior, depth, type/node/draw,
  output, time, and 45,750 sum-of-slide-cubes work identities are retained.
- The planted eight-patient/16-slide/three-type oracle recovers the known type-A group shift, finite
  positive kernel scales, all 192 node/type posteriors, and zero divergences/depth hits. Its direct
  and durable integrations pass in 15.88 and 24.89 seconds. The durable test proves miss, fresh
  backend-disabled hit, byte identity, and one ledger execution through the existing project engine.
- The real durable miss completes in 625.25 seconds at 1,284,538,368-byte maximum RSS. Diagnostics
  are complete: R-hat 1.00782, bulk/tail ESS 479.79/727.01, E-BFMI 0.67294, zero divergences, and zero
  depth hits. Shared amplitude is 1.03498 [0.95688, 1.11971] and length is 286.13 [236.41, 337.82]
  micrometres. Neoplastic, Connective, and Inflammatory MSI-minus-MSS effects are -0.2724
  [-1.1339, 0.5677], 0.0137 [-0.6675, 0.6957], and 0.2224 [-0.8507, 1.2024]; every pairwise
  differential interval also spans zero.
- A fresh backend-disabled process returns a byte-identical hit and the inferred project has one
  execution row. Result SHA-256 is
  `4e437daefdc20133f674877515d911b918f6c7d27380cb3c69d51835771c7e88`; the hash-verified 1-TB
  bundle is `results-cellvit-categorical-v31-replicated-multitype-lgcp-inferred-kernel`.
- Focused warning-denied Clippy, package no-default CLI compilation, Python syntax, affected-file
  Rustfmt, both integrations, bundle verification, and whitespace checks pass. This is one ordinary
  milestone after checkpoint 139, so broad workspace gates were not repeated. No Nextest/full-
  integration loop, feature matrix, benchmark, fuzzing, packaging, dependency audit, push,
  publication, deployment, or history rewrite ran.

## Replicated multitype inferred-kernel agreement checkpoint 141 — 2026-08-29

- Added exact-baseline `replicated-arbitrary-window-multitype-lgcp-inferred-kernel-agreement` and an independent NumPyro 0.21.0/JAX 0.11.1 implementation. The command freshly prepares and validates the complete PyMC request, reads its typed result, then starts only NumPyro. It compares 15 type parameters, two kernel parameters, three pairwise group effects, 24 patient/type, 48 slide/type, 666 latent-node/type, and 666 expected-count quantities.
- The planted-shift integration passes in 32.17 seconds. The real NumPyro fit completes in 438.12 seconds at 1,179,484,160-byte maximum RSS with R-hat 1.00921, bulk/tail ESS 450.57/719.58, E-BFMI 0.67400, zero divergences, and zero depth hits. NumPyro amplitude/length are 1.03231 [0.95201, 1.12134] and 284.68 [234.41, 336.85] micrometres.
- All 1,424 quantities pass and every interval overlaps. Maximum standardized differences are 0.872 for kernel parameters and 0.933 for expected counts. Agreement SHA-256 is `2295cd21b1a836627f1fa39c9082195686ce304865f484f429c533b913ead589`; the hash-verified bundle is `results-cellvit-categorical-v32-replicated-multitype-lgcp-inferred-agreement`.
- Focused warning-denied Clippy, package no-default CLI compilation, Python syntax, affected-file Rustfmt, exact integration, bundle verification, and whitespace checks pass. Broad workspace gates were not repeated two milestones after checkpoint 139. No Nextest/full-integration loop, feature matrix, benchmark, fuzzing, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Replicated multitype inferred-kernel calibration checkpoint 142 — 2026-08-29

- Added exact-request `replicated-arbitrary-window-multitype-lgcp-inferred-kernel-prior-calibration` through pinned NumPy 2.4.6. It independently draws five type/hierarchy prior families plus shared amplitude and physical length, constructs every per-draw/per-slide dynamic Matérn Cholesky, verifies recovered whitened moments and exact within-slide centering, and generates conditional Poisson counts.
- The deterministic small oracle passes. The 4,096-draw exact real-geometry run completes in 1.75 seconds at 565,854,208-byte maximum RSS under a 717,619,200-byte estimate and 1-GiB ceiling. It executes exactly 2,727,936 node-type simulations and 187,392,000 kernel-cube work.
- All seven moment checks pass; the largest mean/SD-relative errors are 0.0354/0.0169. Whitened field mean/second moment are -0.00111/1.00001 with maximum centering error 8.44e-15. Poisson residual mean/second moment are -0.000256/0.99918. Result SHA-256 is `71445bb843aa9ab15ebc0e7a234ad1ebf9861bbbbeae7239e799b9379b23e0fe`; the hash-verified bundle is `results-cellvit-categorical-v33-replicated-multitype-lgcp-inferred-promotion`.
- Final direct, durable, agreement, and calibration integrations pass in 13.80, 24.88, 32.87, and 1.30 seconds. Focused checks pass. At this three-workflow boundary, workspace formatting, all-target/all-feature warning-denied Clippy, workspace no-default compilation, all-feature doctests, strict docs, bundle verification, and whitespace checks pass. The documented Nextest/full-integration loader loop was not run; no feature matrix, benchmark, fuzzing, packaging, dependency audit, push, publication, deployment, or history rewrite ran.

## Sparse radius-heat coordinate stability checkpoint 143 — 2026-08-29

- Added direct and durable `sparse-radius-heat-stability`. It reuses the exact sparse radius-heat
  owner for an unperturbed reference and 1–64 deterministic SHA-256-seeded independent per-axis
  physical coordinate perturbations while fixing node IDs and signals. It reports graph, edge,
  isolate, aggregate work, and relative filtered-signal L2 diagnostics under per-run and
  pre-execution aggregate candidate/matrix-vector ceilings. Non-finite controls/results and
  undefined zero-reference relative changes are rejected.
- The exact zero-jitter identity oracle passes and the underlying sparse transform retains its
  independent dense-eigensolver/negative-grid agreement. The durable integration proves miss,
  fresh-process hit, byte identity, and one execution row. The admitted CPTAC run has 2,000 nodes,
  24,755 baseline edges, nine isolates, and 16 prespecified 1-micrometre jitter replicates. Edge
  counts span 24,705–24,775 and maximum relative filtered-signal change is 0.009872 against the
  fixed 0.10 diagnostic threshold. Observed totals are 955,108 candidate evaluations and
  11,378,068 matrix-vector work.
- The real durable miss completes in 8.94 seconds at 15,482,880-byte maximum RSS; a fresh-process
  hit is byte-identical with one ledger row. Result SHA-256 is
  `fc09927bc10c9f5f8099c2af78b12b554d01a489868ae5fe6db91a399f89d881`; the hash-verified 1-TB
  bundle is `results-cellvit-categorical-v34-sparse-heat-stability`. This remains one-specimen,
  one-signal/radius/time/jitter evidence, not patient reproducibility, segmentation or subsampling
  calibration, molecular association, biological significance, causality, or clinical evidence.
- The affected graph package, legacy sparse direct/durable integrations, and new direct/durable
  integrations pass. Affected files are formatted and the bundle verifies. This is the first
  workflow after checkpoint 142, so broad workspace gates were not repeated; the documented
  Nextest/full-integration loop remains excluded.

## Sparse radius diffusion wavelet checkpoint 144 — 2026-08-29

- Added direct and durable `sparse-radius-diffusion-wavelet`. For 1–16 strictly increasing positive
  diffusion times it constructs the concrete telescoping filter bank `I-H(t0)`, successive
  `H(t[j-1])-H(t[j])`, and final `H(t[last])` through the existing adaptive sparse Chebyshev heat
  owner. It retains only sufficient detail/coarse signals, reports scale energies and approximation
  bounds, and enforces per-scale/aggregate candidate/matrix-vector, peak working, retained-output,
  and shared artifact ceilings.
- The five-node behavior test agrees with independent dense spectral heat at all three test scales,
  reconstructs every intermediate heat signal and the input, and rejects a one-short aggregate
  candidate budget. The durable integration proves miss, fresh-process hit, byte identity, and one
  execution row. Existing sparse-heat and exact diffusion-wavelet integrations remain green.
- The first real complete schema encoded 1,409,419 durable bytes because it redundantly retained
  every heat signal and detail, exceeding the unchanged 1,048,576-byte artifact ceiling. It created
  no execution and is retained. The sufficient filter-bank schema encodes 710,433 bytes. Its real
  durable miss completes in 10.42 seconds at 29,278,208-byte maximum RSS on the same 2,000-node,
  24,755-edge graph at times 0.025/0.05/0.10/0.20. Selected orders are 8/10/13/18, total observed
  candidate/matrix-vector work is 224,344/2,523,990, and reconstruction error is 3.33e-16.
- A fresh process returns the byte-identical hit with one ledger row. Result SHA-256 is
  `2c7ebc610198c39aee8585aeeb6958b9ed1ae821547d01d284e7491a35417d27`; the hash-verified 1-TB
  bundle is `results-cellvit-categorical-v35-sparse-diffusion-wavelet`. This is one-specimen graph-
  signal decomposition, not a sparse eigensolver/basis tree, scattering, patient reproducibility,
  molecular association, biological significance, causal, or clinical evidence. Focused affected
  tests and formatting pass. Broad gates were not repeated two workflows after checkpoint 142.

## Sparse radius scattering checkpoint 145 — 2026-08-29

- Added direct and durable `sparse-radius-scattering`. It composes the DEC-0331 telescoping sparse
  heat filters with pointwise absolute value and graph-node mean/energy aggregation. Fixed order one
  or two is supported over 1–16 increasing times; second-order paths use only strictly coarser
  filters. Exact heat-application planning, aggregate candidate/matrix-vector work, peak graph
  working, retained storage, finite-result, input, output, and durable artifact bounds are explicit.
- The five-node order-two behavior test independently recomputes every required heat action with the
  dense spectral owner, agrees for all three first-order and three second-order coefficients, proves
  the exact eight-application plan, and rejects a one-short aggregate candidate ceiling. The durable
  integration proves miss, fresh-process hit, byte identity, and one ledger row. Existing exact and
  sparse wavelet/scattering integrations remain green.
- The real four-scale 2,000-node/24,755-edge run produces four first-order mean magnitudes
  0.03287–0.05329 and six second-order magnitudes 0.003766–0.008553. All 13 heat applications
  complete with 729,118 candidate evaluations, 8,756,700 matrix-vector work, 2,268,248-byte peak
  estimated graph storage, and 1,288,192-byte conservative retained storage. The identity-final miss
  completes in 10.64 seconds at 20,348,928-byte maximum RSS; its fresh-process hit is byte-identical
  with one ledger row. Result SHA-256 is
  `d1c2a01c709a20528665d21a7d8d45d450f8dc403ae1a06924990514dfab71e0`.
- The hash-verified canonical 1-TB bundle is
  `results-cellvit-categorical-v37-sparse-scattering-final`; v36 is retained as matching pre-final-
  iterator identity evidence. This is one-specimen descriptive graph signal, not patient
  reproducibility, molecular association, biological significance, causal, or clinical evidence.
- At the three-workflow checkpoint, workspace formatting, warning-denied all-target/all-feature
  Clippy, workspace no-default compilation, all-feature doctests, strict all-feature workspace docs,
  focused tests, bundle verification, and whitespace checks pass. Clippy first found and the focused
  tests revalidated one iterator-form cleanup before its final pass. The documented macOS Nextest/
  full-integration loop was not run; no feature matrix, benchmark, fuzzing, packaging, dependency
  audit, push, publication, deployment, or history rewrite ran.

## Witness-persistence coordinate stability checkpoint 146 — 2026-08-29

- Added direct and durable `witness-persistence-stability` through the existing pinned GUDHI 3.13.0
  environment and worker. It fixes exact point IDs and all witness controls, generates 1–32
  deterministic SHA-256-seeded independent per-axis physical perturbations, and reports landmark-ID
  overlap, coverage-radius change, simplex-count L1 change, and per-dimension finite/essential-count
  and total-persistence changes. Backend execution, total point work, aggregate simplex ceiling,
  aggregate timeout, finite-result, source, request, runtime, and artifact bounds are explicit.
- The five-point zero-jitter oracle executes three exact GUDHI requests and requires identity across
  every reported diagnostic; a one-short 15-point aggregate budget is rejected before backend start.
  The durable integration proves miss, fresh-process backend-disabled hit, byte identity, and one
  execution row. Existing direct/durable witness tests remain green.
- The real 2,000-cell/64-landmark, 16-replicate, 1-micrometre diagnostic is truthfully unstable.
  Minimum landmark overlap is 0.890625 against a fixed 0.90 threshold, and maximum per-dimension
  total-persistence change is 43,950.386 square micrometres against 10,000. Maximum coverage change
  1.65024 micrometres and simplex-count L1 change 13 pass their fixed 5/100 thresholds. Total-
  persistence change is not described as bottleneck distance, and thresholds were not changed.
- The remote standalone binary first fails before project creation because its compile-time pinned
  `/Users/user/Bench/gsc-marklab/workers/python/uv.lock` path is absent on the Mac mini. That exact
  stderr is retained. The SHA-identified 2,000-point input then executes in this checkout's existing
  pinned environment in 31.34 seconds at 48,824,320-byte maximum RSS. A fresh backend-disabled
  process returns the byte-identical hit with one ledger row. Result SHA-256 is
  `883d07beed9916a250b41ad57899ecbba31940ae5c0b6096646677fa7cea0326`; the complete project is
  hash-sealed back on the 1-TB drive as `results-cellvit-categorical-v39-witness-stability-final`.
- Focused topology package and all four existing/new direct/durable witness integrations pass;
  affected files are formatted and whitespace checks pass. This is one ordinary workflow after the
  checkpoint-145 stabilization, so broad gates were not repeated and the prohibited Nextest/full-
  integration loop was not run.

## Final CRC scientific analysis checkpoint 147 — 2026-08-29

- `SCIENCE-CRC-FINAL-01` replaces broad master-plan completion as the completed objective. The final
  science lane reuses the admitted CPTAC-COAD eight-patient/16-slide/8,192-cell identity-ranked
  subset, exact 50-micrometre hard-Neoplastic graph signal, raw 1,280-dimensional CellViT tensors,
  existing TCGA M0--M6, pinned M7, Schuerch H&E/CODEX, and CRC outcome bundles. No threshold, subset,
  scale, molecular label, or model was selected from the observed result.
- Every slide now has bounded order-two scattering at 45/50/55 micrometres, deterministic 80% cell
  subsampling, 1-micrometre coordinate perturbation, witness persistence at 180/200/220 micrometres,
  four witness perturbations, and a witness cell-subsample result. Two slides remain nested inside
  each patient. Exact selected tensor rows were recovered from source CellIds and their physical
  coordinates were checked against the allowlisted CellViT graph tensors before nonspatial means
  and population SDs were formed.
- Graph scattering passes cell-subsample, coordinate, and nearby-radius stability summaries but
  fails the frozen two-slide patient gate: median/q10 leave-one-slide patient-rank Spearman are
  0.7857/0.2857 against 0.8/0.6. Witness topology is unstable: only 5/16 slide diagnostics pass all
  coordinate thresholds, cell-subsample and nearby-scale q10 are 0.2417/0.2750, and two-slide
  median/q10 are 0.6535/0.4048.
- All standardization and three-component-per-block PCA are fitted inside each patient-held-out
  training fold. The composition/clinical/nonspatial baseline has balanced accuracy 0.625 with a
  whole-patient bootstrap interval 0.25--0.875 and exact 70-assignment p=0.3429. Adding graph or
  topology changes balanced accuracy and nearest-patient group retrieval by exactly zero. Graph-
  only energy/MMD p-values are 0.485/0.440; topology-only values are 0.906/1.000. Neither block
  passes the strictly-positive incremental-information gate, so neither is added to fusion.
- The final interpretation retains rather than dilutes prior evidence: descriptive short-range
  tumor organization recurs in admitted H&E cohorts, the independent Schuerch H&E CellViT cohort
  supports its frozen direction-level MSI/MSS score, and TCGA M1 has a positive distance-effect
  increment. A stable transferable molecular-class fingerprint is not established: TCGA M6,
  graph/topology increments, Schuerch CODEX class recurrence, and M7 are null-compatible, while
  graph/topology and the existing M2 grid contain explicit instability.
- Graph 80, topology 64, and patient-retrieval 32 unique project executions replay byte-identically
  from fresh backend-disabled processes without new ledger rows. The topology nodes started 128
  exact pinned-GUDHI backend executions on misses. The retained M7 miss/hit SHA-256 remains
  `a0d211414e4a8c0b16c2f682d5bb7f85aa9b2b549996a347b9b36f0c595f847a`.
- The final 1-TB bundle is `/Volumes/1TB/marklab/runs/science-crc-final-01`. All 517 listed artifacts
  rehash successfully; manifest SHA-256 is
  `3bd77f27a3f06f2a59d703a6b0335a51d2ba1b55fda1b76aac10e36e29dd5bf9` and interpretation SHA-256
  is `731668a7693e547f9ccd5d988403321e472242fadc38b5220aa59695b6f5cef2`.
- Focused Python behavior tests, Python compilation, the affected CLI build, remote bundle
  verification, and whitespace checks pass. No Rust production package changed, so affected-package
  Clippy/no-default/docs are not applicable. Checkpoint 145 remains the latest broad workspace
  stabilization; Nextest, full-workspace tests, broad Clippy/docs, feature matrices, benchmarks,
  fuzzing, packaging, dependency audits, push, publication, deployment, and history rewriting were
  not run.

## Sparse component-basis checkpoint 148 — 2026-08-29

- Added direct and durable `sparse-radius-basis` on the existing exact uniform-cell radius graph.
  The method discovers connected components exactly, retains their complete normalized zero
  eigenspace, and approximates only the remaining low modes with deterministic component-wise
  shifted-Laplacian subspace iteration and a bounded small Rayleigh-Ritz solve. Exact node order,
  graph digest, component policy, signal-ignored geometry semantics, native runtime, request, and
  artifact identities are durable.
- Node/candidate/edge/component, matrix-vector, orthogonalization, Ritz-rotation, working-byte, and
  retained-byte ceilings are admitted before iteration. Component-local mode storage keeps the
  sufficient exact-float result below the unchanged 1-MiB project ceiling. Replay validation
  recomputes residual and orthogonality diagnostics and rejects nullspace or eigenvalue-order
  corruption.
- The five-node path CLI agrees with the analytic combinatorial-Laplacian eigenvalues
  `0`, `2-2*cos(pi/5)`, and `2-2*cos(2*pi/5)` within `1e-9`. Unit controls retain both zero modes of
  a disconnected graph, reject one-short matrix-vector work before iteration, prove geometry-only
  invariance to signal changes, and reject corrupted typed diagnostics. Direct/durable basis and all
  affected sparse heat, wavelet, and scattering integrations pass.
- The real 2,000-cell/24,755-edge CPTAC graph has 24 components, including nine isolates. Its
  32-mode basis contains all 24 zero modes and eight nonzero low modes. At 512 fixed iterations,
  maximum residual is `3.3187388269244314e-8` against `1e-4` and orthogonality error is
  `6.5503158452884236e-15`; the identity-final miss takes 8.07 seconds at 28,524,544-byte maximum
  RSS. A 16-mode nullspace truncation, a 256-iteration residual failure, and the initial dense
  result's artifact-
  ceiling failure remain recorded rather than hidden.
- A fresh process reports `cache_status=hit`, emits SHA-256
  `31ab52bb440176ec6989380e28c5a143ec42e15d656ddd678d5c705acf4468a5` byte-identically, and leaves
  one ledger row. The hash-verified 1-TB bundle is
  `results-cellvit-categorical-v43-sparse-radius-basis-final`; `SHA256SUMS` hashes to
  `63ba1b1279eb09476e50872e74d3c15e9d2affc38a0b82bee0c843ee0a931338`.
- Affected formatting, graph-package tests, eight new/legacy CLI integrations, warning-denied
  affected Clippy and graph docs, graph no-default compilation, remote bundle verification, and
  whitespace checks pass. Checkpoint 145 remains the latest broad stabilization; the prohibited
  macOS Nextest/full-integration loop and other broad or specialized gates were not run.

## Sparse Fourier signal-energy checkpoint 149 — 2026-08-29

- Added direct and durable `sparse-radius-fourier-energy` as the first signal-bearing consumer of the
  component-aware sparse basis. It binds an exact sorted scalar-signal digest, reports each signed
  coefficient and squared energy, separates component means from nonzero low-frequency variation,
  and retains total, centered, captured, and unresolved energy with explicit zero-denominator
  states. Conservative node-by-mode projection and summary-byte ceilings are checked before the
  basis runs.
- The independent five-node path oracle builds a signal from a normalized constant mode with
  coefficient 2 and first nonzero analytic eigenmode with coefficient 3. Production recovers total,
  component-zero, and nonzero low-frequency energies 13, 4, and 9 within `1e-8`; a one-short
  projection ceiling rejects before basis execution. Unit coverage retains explicit all-zero and
  component-constant fraction states. The durable integration proves miss, fresh-process backend-
  disabled hit, byte identity, and one ledger row.
- The fixed real hard-Neoplastic signal has 1,450 positive values across the admitted 2,000 nodes.
  Component-zero energy is `1190.7037035326675` of 1,450 total. The eight retained nonzero modes
  contain `21.1297223445104` of `259.29629646733247` within-component centered energy, a fraction of
  `0.08148871631559318`; `238.16657412282206` remains unresolved. The result is retained as mostly-
  unresolved one-specimen truncation evidence, without changing graph radius, signal, modes,
  iterations, tolerance, or thresholds.
- The identity-final miss completes in 7.97 seconds at 25,739,264-byte maximum RSS. A fresh process
  reports `cache_status=hit`; outputs hash byte-identically to
  `e38bec64677b5fbc2ea82eda47dcec15b0ae4d3c7226ec55ef6b5e7076fc192d` with one ledger row. The
  hash-verified 1-TB bundle is `results-cellvit-categorical-v44-sparse-fourier-energy-final`, whose
  `SHA256SUMS` hashes to
  `5784d40fd46568ea0a92495bcba417efb8c3ba98cb77d0f66ef99be84d702b38`.
- Affected formatting, graph-package and direct/durable basis/Fourier tests, warning-denied affected
  Clippy/docs, graph no-default compilation, remote bundle verification, and whitespace checks
  pass. Checkpoint 145 remains the latest broad stabilization; the prohibited macOS Nextest/full-
  integration loop and other broad or specialized gates were not run.

## Patient-replicated sparse Fourier checkpoint 150 — 2026-08-29

- Expanded the bounded basis ceiling from 64 to 128 modes for one demonstrated caller: the frozen
  CRC variants have up to 70 exact components and need at most 78 total modes to retain the complete
  nullspace plus exactly eight nonzero modes. A 130-node/65-component oracle first fails the old
  ceiling, then returns 65 zero and eight nonzero modes under the new cap. Mode count must also not
  exceed node count.
- Added the narrow `marklab_crc_sparse_fourier_patient.py` prepare/execute/summarize CLI. It converts
  the existing 16-slide graph requests without label-based selection, independently audits exact
  components under 9,707,200 of 20,000,000 admitted pair checks, verifies the Rust result counts,
  executes five frozen variants in at most six bounded processes, and keeps two slides nested inside
  each of eight patients. All feature scaling/PCA stays inside each held-out patient fold; the
  existing whole-patient permutation, bootstrap, retrieval, stability, and fusion owners are reused.
- The identity-final runtime SHA-256 is
  `16c6e9e1ad435cdea2fe0c0f2e0c2701a34798451aaae0f65c1d76681bb2205b`.
  Six processes complete 80 misses in 88.00 seconds. The one allowed fresh replay pass completes
  80/80 backend-disabled byte-identical hits in 78.74 seconds; every project ledger remains one row.
  Replay wall time includes the documented repeated macOS binary verification and is not presented
  as eigensolver performance.
- The fixed patient result is unstable and nonincremental. Coordinate-jitter median/q10 Spearman
  pass at 0.9762/0.8906, but cell-subsample values are 0.7306/0.00952, nearby-scale values are
  0.7714/0.1952, and leave-one-slide values are 0.6667/0.3731. Fourier-only balanced accuracy is
  0.50 with exact whole-patient permutation p=0.6286. M0–M3 and M0–M3-plus-Fourier both have balanced
  accuracy 0.625; the increment is exactly zero with whole-patient bootstrap interval
  `[-0.375, 0.375]`. The block fails the frozen fusion gate and is not promoted.
- The final summary SHA-256 is
  `9dc6ad63f7aeb69a7954aab7b81bc2d0b820113bbdb20bd0701f2d229fda5857`. The exact workflow SHA-256
  is `dc3c60323f3fa87ca59ce42bbe662c2f4f074383c4d93a95f11ac9cdb4e988bc`. The 656-file 1-TB bundle
  `results-cellvit-categorical-v46-patient-sparse-fourier-final` rehashes completely; `SHA256SUMS`
  hashes to `fb2b819f4936f6c5e848601618260303645f4139adff1e86df54543fee1cef59`.
- At this third related graph workflow, workspace formatting, warning-denied all-target/all-feature
  Clippy, workspace no-default compilation, all-feature doctests, and strict all-feature workspace
  docs pass. Focused Rust/Python tests and Python compilation pass. The prohibited Nextest/full-
  integration loader loop was not run; no feature matrix, benchmark, fuzzing, memory tool,
  packaging, dependency audit, push, publication, deployment, or history rewrite ran.
- FR-01B/GSP-01/WS-62 close with a negative promotion result. Broader sparse kernels/bands, GPU
  parity, solver/plugin frameworks, and transform catalogs are killed absent a new immediate
  endpoint; instability is not tuned away.

## Witness bottleneck-stability checkpoint 151 — 2026-08-29

- Added direct and durable exact GUDHI bottleneck comparison for the already admitted witness
  perturbation workflow. The typed result distinguishes finite exact distances from an infinite
  essential-count mismatch and preserves the existing total-persistence diagnostic separately.
  Comparison, interval, backend-execution, total-process, timeout, source, request, environment,
  worker, and runtime identities are bounded and durable.
- Independent oracles recover bottleneck distance 1 for intervals `[0,2]` and `[0,3]`, exact zero
  under zero jitter, and typed failure when essential counts differ. One-short comparison admission
  fails before the bottleneck backend starts. Direct and durable focused integrations pass.
- The fixed real 2,000-cell/16-perturbation result is unstable: maximum finite bottleneck distance
  is `7131.385451975762` square micrometres against the unchanged 600-square-micrometre ceiling, and
  at least one comparison has an essential-count mismatch. The pre-existing witness thresholds also
  remain failed. This result is retained without scale, landmark, seed, subset, or threshold tuning.
- The identity-final miss completes in 31.35 seconds at 52,920,320-byte maximum RSS using 17
  witness executions plus one batched bottleneck process. A fresh backend-disabled process emits
  byte-identical SHA-256 `11818914ec96521516a94e03c08710d3a75ff2eba12df16f6b7568003f87d2ef`
  and leaves one ledger row. The 12-file 1-TB bundle
  `results-cellvit-categorical-v48-witness-bottleneck-stability-final` rehashes completely;
  `SHA256SUMS` hashes to
  `77018e4fd3c7f95ec68577cb6b689d9801bf12997aa5b71bd0007788625faa6c`.
- The science-only objective resumes after this interrupted checkpoint. The exact bottleneck result
  is an additional unstable topology diagnostic and is not promoted into the patient fingerprint.
  No general Marklab capability is promoted next.

## SCIENCE-CRC-FINAL-01 canonical bundle checkpoint 152 — 2026-08-29

- Extended only the existing CRC final sealer with optional exact witness-bottleneck evidence. It
  requires typed result identity, byte-identical miss/hit output, one ledger execution, and recorded
  backend-disabled replay before copying the source tree and adding the explicitly one-specimen
  unstable diagnostic. With the option absent, the existing sealer behavior remains unchanged.
- No scientific analysis was rerun. The existing patient-held-out graph/topology results, whole-
  patient permutation/bootstrap, M0--M7, retained pinned Bayesian fit, Schuerch H&E/CODEX evidence,
  outcome evidence, unavailable blockers, fusion gates, and overall interpretation are copied from
  their previously sealed sources.
- The final bundle is `/Volumes/1TB/marklab/runs/science-crc-final-01-v2`. All 529 manifest artifacts
  independently rehash successfully. Manifest SHA-256 is
  `8c03b980b72c2423253e074fc874ce67fe37057707860998f4a27fde180e4e18`; interpretation SHA-256 is
  `d9e8348c9834f6004f5d8656aaa5aeb316ff24d61eb945282cdc41a1555e9c39`.
- The exact bottleneck addendum remains unstable at `7131.385451975762` versus 600 square
  micrometres with essential-count mismatch. Its miss/hit SHA-256 is
  `11818914ec96521516a94e03c08710d3a75ff2eba12df16f6b7568003f87d2ef` with one ledger row. It is
  not a patient replicate and is not fused. The patient conclusion is unchanged: descriptive local
  H&E organization is supported, but no stable transferable molecular-class spatial fingerprint is
  established.
- Focused sealer tests, Python compilation, remote independent manifest rehash, bundle identity,
  replay-byte, ledger-row, formatting, whitespace, diff, and status checks pass. Existing broad
  stabilization evidence was not rerun; no unrelated software outcome is promoted.

## Patient witness-bottleneck checkpoint 153 — 2026-08-29

- Added a narrow prepare/execute/summarize workflow that applies the existing exact GUDHI
  bottleneck project command to the frozen eight-patient/16-slide CPTAC topology subset. Every
  request retains the exact 512 stable-CellId rows, 200-micrometre witness scale, 64 landmarks,
  dimensions 0--2, four deterministic one-micrometre perturbations, seed, and 600-square-
  micrometre threshold. Labels are not used for selection or feature construction.
- The first real preparation is retained as a pre-backend admission failure: 4,000,000 intervals
  were insufficient for 12 comparisons times both diagrams times the 500,000-simplex ceiling. The
  corrected exact 12,000,000 ceiling passes without changing the scientific design. Six bounded
  processes complete 16 misses and 16 fresh backend-disabled hits in 42.48 seconds; all results are
  byte-identical, all ledgers remain one row, and misses use exactly 96 external processes.
- The patient-unit result is unstable and null-compatible. Zero of eight patients passes both
  slides, all eight have at least one essential-count mismatch, and maximum finite patient distance
  is `18807.03660672011` square micrometres. The MSI-minus-MSS mean difference is
  `2972.21834719212`, with whole-patient bootstrap interval
  `[-3738.5436356459386, 10085.458614861542]` and exact 70-assignment p-value 0.60. The block is not
  promoted or fused; instability is not tuned away.
- Summary, patient-row, and execution-manifest SHA-256 values are
  `b9d04d8013bd634f745bb9640b68301139b83034164294883924d328f86026b4`,
  `15be3057dbfd307b2fb73b776a62966e5d3f43baf415cf30f1ac78bc860735d4`, and
  `7bef99dbb4fba9711254a219d5b1de66c6f37adcf394190397026783149ba5d4`.
  The 137-file 1-TB bundle is
  `results-cellvit-categorical-v49-patient-witness-bottleneck-final`; its `SHA256SUMS` hashes to
  `bf6cc998ce8709f766a5ef9c984a28defe8d09265922f16b218f14478ec46e14`.
- Focused patient and existing preparation tests, Python compilation, formatting, remote complete
  rehash, whitespace, diff, and status checks pass. No broad workspace gate is rerun at this
  ordinary milestone.

## Multiclass soft pair-mixing checkpoint 154 — 2026-08-29

- Added public `soft_pair_mixing` and a durable analysis node over the existing complete
  probability-simplex MarkTable column, exact framed observation window, and spatial geometry plan.
  Every directed physical-radius pair contributes the full class outer product. The result retains
  ordered source/target class identities, expected mass, pair probability, exact complete-row
  without-replacement random-label probability, and typed enrichment only for positive null mass.
  No row is thresholded, sampled, renormalized, or collapsed.
- The independent hand oracle uses four complete three-class rows with only A↔B inside radius. It
  recovers observed A→B and B→A probabilities 0.5, null probability 0.125, enrichment 4.0, and A→A
  null probability 1/12. A one-short pair ceiling fails at two visits. Durable execution reports a
  miss then an identical hit with one ledger row.
- Explicit ceilings cover points, classes, simplex values, directed visits, all observed-plus-null
  probability products, retained matrix/geometry memory, scheduler output, and project artifacts.
  Replay validation checks exact request identities, matrix ordering, finite/range/ratio relations,
  aggregate simplex mass within the existing `1e-5` row tolerance, and derived work counts.
- Read-only admission on the authorized representative CPTAC CellViT slide confirms 3,247 cell
  records with keys including `type` and scalar `type_prob`, but no complete class probability
  vector. The paired `.pt` graph contains only embeddings, positions, hard `nuclei_types`, and WSI
  metadata. Real multiclass soft-pair evidence is therefore unavailable; no one-hot or invented
  residual-class vector is substituted.
- The focused new test and affected probability-simplex composition/neighborhood tests pass 7/7.
  Warning-denied affected Clippy, no-default compilation, strict affected docs, formatting, and
  whitespace checks pass. No broad workspace gate runs at this ordinary milestone.

## SCIENCE-CRC-FINAL-01 patient-evidence seal checkpoint 155 — 2026-08-29

- Extended only the existing final CRC sealer with optional patient-replicated exact-bottleneck
  evidence. Before copying, it independently revalidates the frozen patient/slide/request
  identities, two slides per patient, 512 cells per slide, summary/execution identities, every
  miss/hit digest and byte comparison, every one-row project ledger, aggregate backend work, and
  the unstable/no-fusion state. The option-absent sealer path and schema/version remain unchanged.
- No M0--M7, graph, topology, retrieval, cohort, Bayesian, external-validation, or outcome analysis
  is rerun. The already sealed checkpoint-153 source is copied into the final evidence and reported
  separately as unstable and null-compatible rather than promoted or fused.
- The final bundle is `/Volumes/1TB/marklab/runs/science-crc-final-01-v3`. All 666 manifest artifacts
  independently rehash. Manifest SHA-256 is
  `0948a7c49af75de447132dfbabcd3a8b4e702c14e0e997e42b6aef076c1117b4`; interpretation SHA-256 is
  `058c4492f4f9225324d79d7519e3cd5abc6f5ff3637b996401b944b88ace2a4a`.
- The retained patient result remains 0/8 stable, 8/8 with essential-count mismatch, and maximum
  finite distance `18807.03660672011` versus 600 square micrometres. The MSI-minus-MSS effect is
  `2972.21834719212`, with whole-patient bootstrap interval
  `[-3738.5436356459386, 10085.458614861542]` and exact p=0.60. All 16 hits are backend-disabled and
  byte-identical to their misses; all 16 ledgers remain one row.
- Focused sealer tests, Python compilation, real-source validation, remote complete rehash, bundle
  identity, interpretation classification, and ledger-count checks pass. Checkpoint 145 remains the
  requested broad baseline; no broad gate or external backend runs. SCIENCE-CRC-FINAL-01 is
  complete, and no general Marklab capability is promoted next.

## Patient hard-categorical pair checkpoint 156 — 2026-08-29

- Added `marklab project categorical-pair` as the smallest durable CLI connection to the existing
  typed hard-categorical pair node. The command reuses `DurableProject`, the local scheduler and
  artifact store, exact result codec, source/window/config/runtime identities, ledger/recovery, and
  caller-supplied memory/pair/null-work bounds. A fresh process with external backend execution
  disabled returns an identical hit with one ledger row.
- Added the bounded patient prepare/execute/summarize path for the frozen four-MSI/four-MSS,
  two-slide-per-patient CPTAC CellViT subset. Preparation revalidates all 8,192 stable CellIds against
  raw morphology types, exact physical coordinates, and exact patch-union windows. Four declared
  directed tumor–inflammatory/connective pairs run at 20, 50, 100, and 200 micrometres with 99
  complete-row random-label permutations in at most six processes. Labels are not used for
  selection, preparation, pair/radius choice, or within-slide inference.
- The first real pass is retained as a boundary failure: 52 outputs publish while 12 outputs from
  three high-precision windows fail durable decode before ledger commit because a redundant JSON
  perimeter summary moves by one ULP. The complete 11-component window becomes the regression
  oracle. Decode still requires the exact canonical window digest and exact config identity, reports
  the mismatched field, and uses the existing finite comparison only for area/perimeter summaries.
- The identity-final binary SHA-256 is
  `f046d5aea6f0b09c7281ea3d91de9464cf501b1045af94e53cb61779be7087ce`. Six processes complete 64
  misses in 70.28 seconds at 33,456,128-byte maximum RSS. One fresh backend-disabled pass completes
  64/64 byte-identical hits in 64.47 seconds; all 64 ledgers remain one row.
- Twenty-one of 32 declared pair/radius/component endpoints are jointly inference-eligible in every
  slide; 11 predominantly large-radius endpoints remain structurally unavailable. Nested-slide
  rank stability median/q10 is 0.7619/0.4333. Pair-only leave-one-patient-out balanced accuracy is
  0.25 with exact p=0.8857. Adding pairs changes M0–M3 balanced accuracy from 0.625 to 0.375, an
  increment of -0.25 with whole-patient bootstrap interval [-0.625, 0.25]. The smallest step-down
  Max-T adjusted p across 21 endpoints is 0.44. The block is null-compatible, lower-tail unstable,
  nonincremental, and is not promoted or fused.
- The 1,239-file 1-TB bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v50-patient-hard-pair-final`;
  `SHA256SUMS` hashes to
  `c71a28bff748f8d618c2672bfdbd913ffdcd7047d5045e9fab165611a4653e53`. Focused Rust/Python tests,
  Python compilation, affected formatting, warning-denied Clippy, root no-default compilation,
  remote complete rehash, and whitespace checks pass. No broad workspace gate runs.

## Real exact-window inhomogeneous K/L checkpoint 157 — 2026-08-29

- Added `marklab project inhomogeneous-spatial` as the smallest user-facing durable connection to
  the existing PP-02 node. It reuses the exact-float result codec, project, scheduler, artifact
  store, native runtime identity, source/window/config identities, ledger/recovery, and caller-
  supplied point/radius/probe/intensity/pair/null/memory ceilings. The categorical-pair CLI also
  drops its private window reader in favor of the already shared bounded UTF-8 owner.
- The first real exact-window run fails during output validation before ledger commit: a
  high-coordinate grid centre built as `min + index*spacing + 0.5*spacing` differs bitwise from the
  validator's algebraically equivalent reconstruction. The durable CLI oracle is changed to retain
  the real high-coordinate rectangle, fails with `fixed intensity grid row is inconsistent`, and
  passes after validation uses the builder's exact operation order.
- The identity-final run uses the first provenance-sorted frozen CPTAC slide: 512 exact cells, its
  12-component patch-union window, 20/50/100-micrometre radii, 50-micrometre Gaussian leave-one-out
  bandwidth, a fixed 16x16 grid, 19 conditioned simulations, seed 20260829, and explicit resource
  ceilings. It completes in 6.26 seconds at 22,118,400-byte maximum RSS. A fresh backend-disabled
  process returns a byte-identical hit; SHA-256 is
  `26861081dda3ee587d20aa0f9fe79056cc1676b46cbfeaebdb981fc2918654d5` and the ledger remains one row.
- L-minus-r values are -18.686, +16.992, and +22.135 micrometres; all radii are jointly eligible.
  The global 19-draw p=0.05 is the smallest attainable value and is not treated as evidence. Only
  37/256 bounding-grid centres fall inside the disconnected window, fixed-grid mass is 205.30 for
  512 cells, and fitted event intensity spans 4.36e-9 to 4.22e12 per square micrometre. This
  configuration is retained as an unreliable real-scale/capacity diagnostic without tuning and is
  not promoted.
- The 17-file 1-TB bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v51-inhomogeneous-kl-final`;
  `SHA256SUMS` hashes to
  `5e2af34ed49a52d1a044ae8f75442ad622f9477c0e0b311c417bbbdfad00cd74`. Six affected direct/durable
  tests, warning-denied Clippy, no-default compilation, affected formatting, remote rehash, and
  whitespace checks pass. No broad workspace gate runs.

## SCIENCE-CRC-FINAL-01 final seal checkpoint 158 — 2026-08-29

- The previously interrupted witness-persistence work was already complete at checkpoints 151 and
  155: 16 patient misses and 16 backend-disabled byte-identical hits, one row per ledger, with the
  observed 0/8 stable result and essential-count mismatches retained outside fusion. It was not
  rerun.
- Extended only the final CRC sealer with optional patient hard-pair evidence completed after the
  v3 seal. The sealer independently revalidates eight patient and 16 nested-slide identities,
  prepared-input digests, four prespecified pair families, 64 miss/hit digests and byte comparisons,
  64 one-row ledgers, six-process bound, backend-disabled replay, fold-internal preprocessing,
  whole-patient inference, stability, incremental information, and the acquisition-site blocker.
- The final bundle is `/Volumes/1TB/marklab/runs/science-crc-final-01-v4`. All 1,905 manifest
  artifacts independently rehash and 1,906 files include the manifest. Manifest SHA-256 is
  `1fe3dbeec3de956f2cdd6a9e67f8ccd983ae26301bfe44383fbb42a525b4f2b2`; interpretation SHA-256 is
  `34f29d358deeac520facfd0cd644fc800461c2af049e89424d0d053ed35023ed`.
- The added pair block remains null, lower-tail unstable, and nonincremental: pair-only balanced
  accuracy 0.25, exact patient-label p=0.8857, increment beyond M0--M3 -0.25 with interval
  [-0.625, 0.25], nested-slide median/q10 0.762/0.433, and minimum step-down adjusted p=0.44. It is
  not fused. The final conclusion remains that descriptive local CRC H&E organization is supported,
  while a stable transferable molecular-class spatial fingerprint is not established.
- Focused sealer/patient tests, Python compilation, one remote seal, independent complete manifest
  rehash, bundle identity, interpretation, replay-count, ledger-count, formatting, whitespace,
  diff, and status checks pass. No completed scientific analysis, Bayesian backend, or broad
  checkpoint gate reruns. SCIENCE-CRC-FINAL-01 is complete; no general capability is promoted.

## Real exact-window inhomogeneous pair-correlation checkpoint 159 — 2026-08-29

- Added `marklab project inhomogeneous-pair-correlation` over the existing typed PP-03A node. It
  reuses the exact K/L persisted pilot, source/window/config/runtime identities, scheduler, durable
  project, artifact store, ledger/recovery, exact-float result codec, and explicit probe/intensity/
  pair/null/memory bounds. No estimator or project infrastructure is duplicated.
- Red-first CLI evidence fails solely because the project subcommand is absent. The completed test
  proves a fresh miss, a fresh backend-disabled hit, byte equality, typed kernel/result content,
  and one ledger row on a high-coordinate exact-window regression.
- The identity-final real run uses the same frozen 512-cell CPTAC slide and 12-component exact
  window as checkpoint 157, with 20/50/100-micrometre radii, 50-micrometre intensity bandwidth,
  10-micrometre pair bandwidth, fixed 16x16 grid, 19 simulations, and seed 20260829. The miss takes
  8.07 seconds at 22,790,144-byte maximum RSS; the backend-disabled hit takes 5.66 seconds. Both
  hash to `46539d2263b16abf0dd88be2b8e54961788a03be51b9f0bf1a1a261ee0b98d3a`, and the ledger has one row.
- Estimated g is 0.002930, 1.215872, and 0.898092. All radii are eligible, but the global p=0.05 is
  the smallest possible with 19 draws. Only 37/256 probes lie inside the disconnected window,
  fixed-grid mass is 205.30 for 512 cells, and intensity spans 4.36e-9 to 4.22e12 per square
  micrometre. The result is retained as unreliable real-scale/capacity evidence, not inhibition.
- The 12-file 1-TB bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v52-inhomogeneous-g-final`;
  `SHA256SUMS` hashes to
  `1a3a9b13110d3d19df91f8a2960410fec3d0d05de1d05cb3a40e495e84b6bc63`. Four focused direct/durable
  tests, warning-denied affected Clippy, no-default compilation, affected formatting, remote
  complete rehash, whitespace, diff, and status checks pass. No broad workspace gate runs.

## Durable real categorical cross-g checkpoint 160 — 2026-08-29

- Added `marklab project categorical-cross-pair-correlation` over the existing typed PP-03B node.
  Red-first CLI evidence fails only because the subcommand is absent; the completed test proves a
  fresh miss/hit across processes, backend-disabled replay, byte equality, typed source/target/
  kernel/curve content, and one ledger row.
- Extracted the exact categorical project input adapter now used by both `categorical-pair` and
  cross-g: stable CellIds, codebook/codes, patient/slide hierarchy, physical frame, typed MarkTable
  provenance, source identities, bounded durable project/store opening, and create-new output. The
  scientific nodes, configs, schemas, nulls, and codecs remain separate. A fresh v50-identity pair
  run is byte-identical to the sealed v50 artifact with SHA-256
  `4045fdbfffc0579e74ef009842eccbabf5a20a2bc13f7356cd31d9ea77ad46aa`.
- The fixed real caller uses the first frozen 512-cell CPTAC slide, its exact 12-component window,
  Neoplastic-to-Inflammatory direction, 20/50/100/200-micrometre radii, 10-micrometre bandwidth, 19
  complete-row permutations, and seed 20260829. The miss takes 5.96 seconds at 28,868,608-byte
  maximum RSS; the backend-disabled hit takes 5.67 seconds. Both hash to
  `f36687f80cd3570300f1fc7416a4bf00807e8c74308fe96c765b4874c9fa2634`, with one ledger row.
- Cross-g is 0.5912 and 0.6037 at the inference-eligible 20/50-micrometre endpoints; global p=0.20.
  The 100-micrometre value 2.0257 has only three eligible source centers and no envelope; 200
  micrometres is typed unavailable with no eligible centers. The result is one-specimen,
  null-compatible, and not promoted as patient evidence.
- The 24-file 1-TB bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v53-cross-g-final`; `SHA256SUMS` hashes to
  `1e4a8d0186ce76f5daa853f3fcfef6d0e4129f2b3fb3b384804aa86f68bd9797`. Seven focused new/oracle/
  regression tests, warning-denied affected Clippy, no-default compilation, affected formatting,
  complete remote rehash, replay/parity comparisons, whitespace, diff, and status checks pass. No
  broad workspace gate runs.

## Patient categorical cross-g checkpoint 161 — 2026-08-29

- Parameterized the existing categorical patient workflow only at its statistic boundary. The
  default categorical-pair command, schemas, preparation, results, and v50 bytes remain unchanged;
  cross-g reuses the frozen design, bounded six-process execution, patient nesting, endpoint
  intersection, fold-internal held-out evaluation, whole-patient permutation/bootstrap, Max-T, and
  leakage blockers.
- The behavior oracle now runs both statistics: 64 fake misses plus 64 backend-disabled hits each,
  byte equality, one-row ledgers, structural endpoint removal, patient reduction, held-out increment,
  explicit cross-g nonpromotion/fusion, and patient Max-T. The exact affected 13-test Python suite
  and compilation pass.
- The real 64 cross-g misses complete in 69.93 seconds at 31,244,288-byte parent maximum RSS. The
  single backend-disabled replay pass completes 64 hits in 64.17 seconds at 33,554,432-byte maximum
  RSS. Every result is byte-identical, all 64 ledgers remain one row, and the binary SHA-256 is
  `b45b5afe3f2ecd13907ab4479773d04f6a681ea793731e5e78828786ca7bbe00`.
- Eight of 16 endpoints are complete across every slide. Nested-slide median/q10/minimum stability
  is 0.762/0.512/0.405. Cross-g-only balanced accuracy is 0.125 with exact p=0.9714. Adding cross-g
  changes M0--M3 balanced accuracy from 0.625 to 0.375, an increment of -0.25 with whole-patient
  interval [-0.625, 0]. Minimum step-down adjusted p is 0.921. The block is explicitly
  nonincremental, not promoted, and not fused.
- The 688-file 1-TB bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v54-patient-cross-g-final`;
  `SHA256SUMS` hashes to
  `d094671df0d0dd01e912322ba2c99afe3a4c90c73032d529530bde7e67f8b4b6`. Remote complete rehash,
  all 64 replay-byte comparisons, 64 one-row ledgers, summary promotion identity, formatting,
  whitespace, diff, and status checks pass. No Rust package changes in this checkpoint, so affected
  Clippy/no-default checks are not applicable; checkpoint 160 retains the current Rust evidence.

## Point-process/mark stabilization checkpoint 162 — 2026-08-29

- Stabilized the related production run comprising complete-simplex pair mixing, patient hard-pair
  inference, real inhomogeneous K/L and g, durable categorical cross-g, and patient cross-g. No
  retrospective cleanup or behavior change is required after direct diff review.
- `cargo +1.96.0 fmt --all --check` passes. `cargo +1.96.0 clippy --locked --workspace
  --all-targets --all-features -- -D warnings` passes in 49.66 seconds without findings.
  `cargo +1.96.0 check --locked --workspace --no-default-features` passes in 1.44 seconds.
- `cargo +1.96.0 test --locked --workspace --doc --all-features` passes all 17 package doctest
  binaries in 14.20 seconds. `RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace
  --all-features --no-deps` passes in 4.27 seconds.
- The documented macOS Nextest/full-integration loader loop is not retried. The complete feature
  matrix is reserved by `WORKSPACE_POLICY.md` for phase/release boundaries and is not run here.
  Focused behavior, real miss/hit, remote rehash, and patient-unit evidence remain recorded at
  checkpoints 154/156/157/159/160/161. No successful expensive gate is repeated.
- Final formatting, whitespace, direct diff, and status checks pass. No benchmark, fuzzing, DHAT,
  RSS, packaging, dependency audit, push, publication, deployment, or history rewrite runs.

## Piecewise binary-compartment intensity/K-L checkpoint 163 — 2026-08-29

- Added `analyze_piecewise_compartment_spatial_pattern`, its durable typed node, and `marklab
  project piecewise-compartment-spatial` as the second explicit PP-05 estimator and immediate PP-02
  K/L caller. It reuses the exact oriented `BinaryCompartmentPartition2D`, shared standard-border
  inverse-intensity pair accumulator, scheduler, artifact store, ledger/recovery, native runtime
  identity, exact-float codec, and result-format 0.3 compatibility shell.
- Each observed event persists `(n_c-1)/area_c`, its exact role/compartment/count/training-count,
  and the oriented partition digest. Both compartments require at least two events; interface
  events are rejected. Null patterns fix the two observed compartment counts and sample uniformly
  within the corresponding exact polygon windows under a new deterministic seed namespace. Point,
  radius, membership-query, pair-visit, null-draw, boundary-segment/input, and retained-memory
  ceilings are explicit.
- The unequal-count rectangle oracle uses two stroma and three tumor events over equal 50-square-
  micrometre areas, independently loops over boundary-eligible directed pairs, and agrees on every
  center/pair inverse-intensity sum and K/L value. Interface, sparse-compartment, one-short query,
  and one-short null-draw failures are covered. Durable evidence proves deterministic replay, seed
  and role-order invalidation, fresh-process miss/hit byte identity, and one ledger row.
- Bounded read-only admission checks find only whole `window.geojson` artifacts in
  `science-crc-final-01-v4` and no mask/compartment/GeoJSON files in the admitted v25 multitype
  input directory. A real piecewise run is therefore unavailable because no provenance-complete
  segment-aligned binary polygon tessellation exists; categorical CellViT labels are not converted
  into invented geometry.
- Twelve focused new/affected K/L/g tests pass. Warning-denied affected Clippy, root no-default
  compilation, strict affected docs, affected formatting, and whitespace checks pass. Checkpoint
  162 remains the latest broad stabilization; no workspace-wide/Nextest loop, feature matrix,
  benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history rewrite
  runs.

## Piecewise binary-compartment pair-correlation checkpoint 164 — 2026-08-29

- Added `analyze_piecewise_compartment_pair_correlation`, its durable typed node, and `marklab
  project piecewise-compartment-pair-correlation`. The result reuses exactly the checkpoint-163
  `(n_c-1)/area_c` event artifact, fixed negative/positive-count exact-window null samples, role/
  interface/sparse policy, deterministic seed namespace, and hard query/pair/draw/memory limits,
  while binding a distinct positive Epanechnikov pair bandwidth below every requested radius.
- Extracted only requirements now shared by both immediate callers: bounded four-source partition
  preparation and create-new output, exact null sampling, compact-support g accumulation, and g
  curve validation. K/L and g retain separate public configs/results/schemas/nodes; the Gaussian
  estimator/result family and result-format 0.3 remain unchanged.
- On the unequal-count rectangle, an independent direct loop reports two boundary-eligible centers
  and four directed pairs at radius 2 and bandwidth 0.5, then agrees with production on the inverse-
  intensity center/kernel sums and normalized g. Durable tests prove pair-bandwidth invalidation.
  Fresh CLI processes return a miss then backend-disabled byte-identical hit with one ledger row.
- Sixteen focused piecewise/Gaussian K/L/g direct, durable, and CLI tests pass after the shared
  extraction. Warning-denied affected Clippy first reports one complex sampled-pattern tuple; after
  replacing it with a named internal type, the three piecewise targets pass 8/8 and the exact Clippy
  command passes. Root no-default compilation, strict affected docs, affected formatting, and
  whitespace checks pass.
- The checkpoint-163 admission result remains authoritative: no provenance-complete exact binary
  compartment polygons exist in the admitted CRC artifacts, so no real piecewise g result is run or
  fabricated. Checkpoint 162 remains the latest broad stabilization; no workspace-wide/Nextest
  loop, feature matrix, benchmark, fuzzing, packaging, dependency, push, publication, deployment,
  or history rewrite runs.

## Prespecified Gaussian bandwidth-selection checkpoint 165 — 2026-08-29

- Added `analyze_selected_inhomogeneous_spatial_pattern`, its durable node, and `marklab project
  gaussian-bandwidth-selected-spatial`. Callers must supply a strictly increasing bandwidth list.
  Every candidate is scored only by mean log boundary-corrected leave-one-out event intensity;
  exact ties retain the smallest bandwidth. Every score, intensity range, evaluation count,
  candidate/config/artifact identity, and the explicit false `selection_uses_spatial_curve` flag is
  persisted before the existing Gaussian K/L workflow runs once at the selected bandwidth.
- Selection candidate count and aggregate intensity evaluations have separate hard ceilings; the
  selected analysis retains its existing probe/intensity/pair/draw/memory limits. Candidate-list
  changes invalidate durable identity. A shared point-table/window/project preparation and
  create-new output boundary is extracted only after fixed Gaussian K/L, Gaussian g, and selected
  K/L all require it; existing store IDs, schemas, nodes, and result bytes remain separate.
- An independent four-point/10x10-grid Gaussian loop agrees with all three candidate scores and
  selects 1 micrometre from the fixed `[1,2,3]` list. A one-short aggregate selection-evaluation
  limit fails. Fresh CLI processes return a miss then backend-disabled byte-identical hit with one
  ledger row; durable candidate-list invalidation is verified.
- The real CPTAC diagnostic is not rerun or tuned. Its retained 16x16 pilot contains only 37/256
  in-window probes and already fails fixed-grid mass/intensity-range reliability, so bandwidth
  selection cannot make that quadrature admissible. `/opt/homebrew/bin/Rscript` exists, but
  `packageVersion("spatstat.explore")` fails because the package is absent and no repository-pinned
  R environment exists. Pinned external agreement remains an exact backend/environment blocker.
- Thirteen focused selected/fixed Gaussian K/L/g direct, durable, and CLI tests pass after shared
  preparation extraction. Warning-denied affected Clippy finds and resolves one needless borrow and
  one boolean assertion form before its final pass. Root no-default compilation, strict affected
  docs, affected formatting, and whitespace checks pass. No broad gate or unpinned package install
  runs.

## PP-05 estimator-family stabilization checkpoint 166 — 2026-08-29

- Stabilized the three related post-checkpoint-162 workflows: exact binary-compartment
  inhomogeneous K/L, the same estimator/null through compact-support g, and prespecified Gaussian
  leave-one-out-likelihood bandwidth-selected K/L. Direct review finds no retrospective production
  cleanup or behavior change required.
- `cargo +1.96.0 fmt --all --check` passes. Workspace all-target/all-feature warning-denied Clippy
  passes in 56.09 seconds without findings. Workspace no-default compilation passes. All-feature
  doctests pass all 17 package binaries, and strict all-feature workspace docs pass.
- The documented macOS Nextest/full-integration discovery loop is not run. The phase/release-only
  compile matrix remains inapplicable. Focused behavior, CLI replay, exact blocker, and real
  nonpromotion evidence remain recorded at checkpoints 163--165. No benchmark, fuzzing, packaging,
  dependency audit, push, publication, deployment, or history rewrite runs.
- Final formatting, whitespace, direct diff, and status checks pass. The user's pre-existing CRC
  outcome and README/package metadata work remains unstaged and unmodified.

## Translation-corrected polygon-window K/L checkpoint 167 — 2026-08-29

- Added `analyze_translation_spatial_pattern`, its strict version-one document, durable typed node,
  and `marklab project translation-spatial`. The existing standard-border classical family and
  result-format 0.3 are unchanged. The estimator evaluates each unordered displacement once,
  contributes both ordered `area(W)/area(W intersect (W+h))` weights, normalizes by `n(n-1)`, and
  uses the existing whole-pattern conditional-CSR/ERL owners under a distinct seed namespace.
- `ObservationWindow2D` now retains one geometry derived from its already validated canonical
  rings. `geo` 0.33.1 is admitted with default features disabled for Boolean intersection and
  planar area only; its lock-compatible `geo-types` is 0.7.19, preserving the existing
  `thiserror` 2.0.18 selection. Point/radius/pair/overlap-call/conservative segment-pair/output-
  vertex/CSR-draw/retained-memory ceilings are cache-bound. Zero-measure and nonfinite overlap,
  translated-coordinate overflow, and output-complexity exhaustion fail explicitly.
- The 10-by-10 rectangle gives overlap 90 and K `10000/90`; a concave seven-unit L gives overlap 3
  and K `49/3`. A static GEOS 3.14.1 holed, disconnected multipolygon fixture independently gives
  area 93 and translated overlap 61.8125. Exact 20/19 pair and overlap edges, 320/319 candidate
  work, preflight output, one-byte-short memory, strict codec corruption, cache invalidation, and
  fresh-process miss/backend-disabled hit byte identity are covered.
- A bounded capacity run reuses the frozen 512-cell CPTAC input and 12-component exact window from
  checkpoint 157 at one prespecified 20-micrometre radius, 19 simulations, seed 20260829, and exact
  ceilings. The miss completes in 6.68 seconds at 22,528,000-byte maximum RSS; the fresh disabled-
  execution hit completes in 5.76 seconds. Both results hash to
  `336b9ce32d50dd9b3d782506ff574bdb52fc0153f88afde4b0be83ccb5e6552f`, and the ledger has one row.
  Observed L is 30.826 micrometres with p=0.10; this is one-specimen capacity evidence and is not
  interpreted as interaction or patient evidence.
- Five final new behavior tests pass after the source/runtime correction; the earlier affected
  classical-plus-translation command passes 39/39. Warning-denied affected Clippy, root no-default
  compilation, strict affected docs, GEOS fixture regeneration, affected formatting, and
  whitespace checks pass. No workspace-wide/Nextest loop or unrelated broad gate runs.

## Isotropic visible-arc K/L checkpoint 168 — 2026-08-29

- Added `analyze_isotropic_spatial_pattern`, a strict version-one isotropic document, durable typed
  node, and `marklab project isotropic-spatial`. Every unordered pair is inspected once; each
  eligible direction uses the reciprocal fraction of its distance circle visible in the exact
  window, then K is `area * directed_weight_sum / (n*(n-1))`. The existing conditional-CSR/ERL,
  project, scheduler, store, ledger, recovery, raw source, parsed geometry, and runtime owners are
  reused under a distinct seed and result identity. Border, translation, and format-0.3 bytes are
  unchanged.
- `ObservationWindow2D` now computes visible fractions by analytic segment-circle intersection
  angles followed by exact-window midpoint classification of each open arc. Shared-vertex/tangent
  angles are tolerance-deduplicated; fractions within numerical endpoint tolerance are clamped to
  `[0,1]`. Point/radius/unordered-pair/directed-arc/segment-test/membership-query/CSR-draw/memory
  ceilings are explicit and cache-bound. Nonpositive visible measure, finite failures, and work
  exhaustion fail before output.
- Independent hand oracles give directed fractions `2/3` and `1` in a rectangle, `5/6` and `11/12`
  around a square hole, and `1/4` and `1/2` for boundary-centered circles. A one-million-angle
  concave-window sampler agrees independently. Exact 20/19 pair, 40/39 arc, 160/159 segment,
  one-short membership and memory boundaries pass; a tangent-only boundary pair is rejected.
- Canonical JSON normalization revealed and fixed a one-ULP first-round-trip drift. Both isotropic
  and translation documents now emit a bounded stable numeric fixed point, preserving the existing
  stable translation bytes and guaranteeing isotropic miss/hit byte identity. Strict unknown/value
  corruption and arc-limit cache invalidation are covered.
- The final bounded real run reuses checkpoint 157's frozen 512 cells and 12-component window at
  radius 20, 19 simulations, seed 20260829, and exact ceilings. Its miss completes in 6.28 seconds
  at 22,478,848-byte maximum RSS; a fresh backend-disabled hit completes in 5.78 seconds at
  21,659,648 bytes. Both hash to
  `98961c7825ae78f3805b3f643494607a83f0600edaf0582266b4cdc8cf6c0ceb` with one ledger row.
  L is 31.084 micrometres and p=0.10; this is capacity evidence, not biological or patient evidence.
- The final affected classical/isotropic/translation command passes 24/24. Warning-denied affected
  Clippy and root no-default compilation pass. Strict docs first reject one accidental `[0,1]`
  intra-doc link and pass after the literal is backticked. Affected formatting, whitespace, diff,
  and status checks pass; no broad workspace/loader loop runs.

## Translation-corrected homogeneous pair correlation checkpoint 169 — 2026-08-29

- Added `analyze_translation_pair_correlation`, a strict standalone version-one document, durable
  node, and `marklab project translation-pair-correlation`. Every unordered displacement in at
  least one strict Epanechnikov support interval evaluates the existing exact polygon overlap once;
  represented radii receive both ordered kernel-weighted `area(W)/overlap` factors and
  `area(W)/(2*pi*r*n*(n-1))` normalization. Conditional CSR and ERL use a distinct seed namespace.
- Translation K/L and g now share only exact overlap work accounting and preflight through
  `translation_spatial::edge`; window geometry, scheduler, store, ledger, recovery, source/runtime
  identities, and output transactions remain existing owners. Standard-border g, translation K/L,
  isotropic K/L, and result-format 0.3 bytes are unchanged. No correction registry or automatic
  selection surface is introduced.
- A two-point rectangle gives overlap 90, weighted kernel sum `10/3`, and g `250/(3*pi)`. The static
  GEOS 3.14.1 holed/disconnected multipolygon fixture independently gives overlap 61.8125 and agrees
  with the complete normalized formula. Strict compact-support endpoints are unavailable rather
  than zero; exact 20/19 pair, 20/19 overlap, 320/319 conservative Boolean work, preflight output,
  one-byte-short memory, strict corruption, and cache-limit invalidation are covered.
- The bounded real run reuses checkpoint 157's hash-verified frozen 512-cell/12-component CPTAC
  input at radius 20, the already recorded 10-micrometre pair bandwidth, 19 simulations, and seed
  20260829. The miss takes 7.16 seconds at 23,379,968-byte maximum RSS; a fresh backend-disabled
  hit takes 5.80 seconds at 21,626,880 bytes. Both outputs hash to
  `8bc1bfbf211e6b474fe723534fd1f6d4b1eaffbf2d81984e4af691114cce7126`, `cmp` passes, and the ledger
  has one row. Observed g is 2.109894 with p=0.10; this is one-specimen capacity evidence only.
- Fourteen focused direct/durable standard-border and translation K/L/g tests pass, followed by the
  fresh-process translation-g CLI test and both seed namespace tests. Warning-denied affected
  Clippy, root no-default compilation, strict affected docs, affected formatting, and whitespace
  checks pass. No workspace-wide/Nextest loader loop or unrelated broad gate runs.

## Isotropic homogeneous pair correlation checkpoint 170 — 2026-08-29

- Added `analyze_isotropic_pair_correlation`, its strict version-one document, durable node, and
  `marklab project isotropic-pair-correlation`. Each compact-support unordered displacement
  evaluates both exact visible-circle fractions once, reuses them across represented radii, and
  accumulates the Epanechnikov kernel times both reciprocal fractions before
  `area(W)/(2*pi*r*n*(n-1))` normalization. Conditional CSR/ERL has a distinct seed namespace.
- Isotropic K/L and g share only visible-arc work accounting; exact window geometry, project,
  scheduler, store, ledger, recovery, source/runtime identity, and transaction owners are reused.
  Existing border/translation/isotropic result bytes and result-format 0.3 are unchanged.
- A rectangle with directed fractions `2/3` and `1` gives weighted kernel sum `15/4` and g
  `375/(8*pi)`. A square-hole oracle with fractions `5/6` and `11/12` agrees independently. Strict
  kernel endpoints and exact 20/19 pair, 40/39 arc, 160/159 segment, one-short membership, and
  one-byte-short memory boundaries pass.
- The hash-verified checkpoint-157 512-cell/12-component input runs at radius 20, bandwidth 10, 19
  simulations, and seed 20260829. The miss takes 8.32 seconds at 22,528,000-byte RSS; the fresh
  backend-disabled hit takes 5.79 seconds at 21,708,800 bytes. Both hash to
  `dc082a26b2b144f563151acd42a3423c296f57cdf85c25cbfe039abbd211bd8b` with one ledger row. Observed
  g is 2.135402 and p=0.10; this is one-specimen capacity evidence only.
- Nine focused isotropic K/L/g direct and durable tests plus the fresh-process g CLI test pass.
  Warning-denied affected Clippy, root no-default compilation, strict docs, affected formatting,
  and whitespace checks pass. No workspace-wide/Nextest loop or unrelated broad gate runs.

## Corrected point-process stabilization checkpoint 171 — 2026-08-29

- Stabilized the four related post-checkpoint-166 workflows: exact polygon translation and
  visible-arc isotropic homogeneous K/L plus their compact-support g callers. Retrospective direct
  review finds the correction identities separate, shared code limited to the two proven geometry
  work budgets, and no result-format 0.3, standard-border, project, scheduler, store, or transaction
  change requiring cleanup.
- `cargo +1.96.0 fmt --all --check`, warning-denied workspace all-target/all-feature Clippy,
  workspace no-default compilation, all-feature workspace doctests, and strict warning-denied
  all-feature workspace docs pass once without findings.
- The documented macOS Nextest/full-integration discovery loop is not retried. Focused direct,
  durable, CLI, exact geometry, real-capacity, and backend-disabled replay evidence remains in
  checkpoints 167--170. No benchmark, fuzzing, packaging, dependency, publication, deployment,
  push, or history rewrite runs.

## Translation-corrected directed categorical cross-g checkpoint 172 — 2026-08-29

- Added `translation_categorical_cross_pair_correlation`, its store-aware node, and `marklab project
  translation-categorical-cross-pair-correlation`. Typed labels, declared direction, bandwidth,
  complete-row random labeling, and ERL remain intact; exact overlap is evaluated once per retained
  contributing unordered displacement under a separate result/cache identity.
- The rectangle oracle gives one source-to-target pair, overlap 24, weighted sum `7/4`, and cross-g
  `49/(8*pi)`. Direct replay, durable miss/hit/seed invalidation, and fresh-process disabled replay
  pass with standard-border regressions unchanged.
- The frozen 512-cell Neoplastic-to-Inflammatory capacity miss takes 7.96 seconds at 25,673,728-byte
  RSS; the hit takes 5.83 seconds at 22,872,064 bytes. Both hash to
  `a6f18a64141d8f93c460f67e753591c37836c5208abb29a195c4651e08a625e8` with one ledger row.
  Counts are 140/26, cross-g 0.655655, and p=0.10; this is not patient evidence.
- Six direct/durable tests and both CLI targets pass. Warning-denied affected Clippy, root
  no-default compilation, strict docs, formatting, and whitespace checks pass. No broad gate runs
  after checkpoint 171.

## Isotropic directed categorical cross-g checkpoint 173 — 2026-08-29

- Added `isotropic_categorical_cross_pair_correlation`, its store-aware node, and `marklab project
  isotropic-categorical-cross-pair-correlation`. Exact fractions are evaluated twice per retained
  contributing unordered displacement; the source-centred fraction supplies each directed
  kernel/fraction weight through complete-row random labeling and ERL.
- A boundary-source rectangle gives fractions `1/2` and `1`, weighted sum 4.5, and cross-g `9/pi`.
  Direct, durable miss/hit/seed invalidation, and fresh backend-disabled replay pass. The full typed
  target passes 8/8 and all three categorical cross-g CLI targets pass.
- The frozen 512-cell Neoplastic-to-Inflammatory miss takes 7.90 seconds at 25,165,824-byte RSS;
  the hit takes 5.83 seconds at 23,494,656 bytes. Both hash to
  `73036c3fb94b2194a726e7f03c2a59429e82d1790004aaef5d918d06544ca395` with one ledger row.
  Cross-g is 0.604679 and p=0.10; this is one-specimen capacity evidence only.
- Warning-denied affected Clippy, root no-default compilation, strict docs, formatting, and
  whitespace checks pass. No broad gate runs after checkpoint 171.

## Corrected categorical patient sensitivity checkpoint 174 — 2026-08-29

- The existing eight-patient/16-slide CellViT categorical executor now selects the translation and
  isotropic directed cross-g project workflows with correction-specific exact geometry ceilings.
  Population inference remains at the patient unit: slides are nested diagnostics, preprocessing
  is fit inside each leave-one-patient-out fold, molecular labels are permuted as whole patients,
  and the same prespecified positive-increment fusion gate is retained.
- On the authorized CPTAC subset, each correction completed 64 durable misses and 64 fresh-process
  backend-disabled hits with byte-identical results and one execution per ledger. Translation took
  106.71/89.33 seconds at 31,129,600/23,609,344-byte RSS; isotropic took 91.93/87.94 seconds at
  32,784,384/23,625,728-byte RSS. The deployed binary and worker hash to
  `10c7de4877b060cef4c3a770af4d43aed8fa786240a79c94ebe9f1439338c53d` and
  `10ac6df0aac124ff42363214818b238ec66b3c537dd5f0eb24aeb649276293ea`.
- Translation nested-slide rank stability has median 0.8452 and q10 0.55; isotropic has median
  0.8095 and q10 0.5024. Both add 0.0 held-out balanced accuracy and -0.125 retrieval accuracy
  beyond M0/M3, with whole-patient bootstrap interval [-0.375, 0.375]. Neither is promoted or
  fused. Across all 16 endpoints, translation adjusted p-values are at least 0.454 and isotropic
  adjusted p-values at least 0.332; these null-compatible, nonincremental results are retained
  without scale, endpoint, subset, threshold, or correction selection.
- The focused Python patient integration passes 1/1 in 7.26 seconds. The correction-specific
  manifests and summaries are sealed on the authorized 1 TB drive; no broad workspace gate is
  repeated after checkpoint 171.

## SCIENCE-CRC-FINAL-01 read-only completion audit checkpoint 175 — 2026-08-29

- A bounded reconciliation confirms the requested witness checkpoint and every remaining
  science-critical lane were already completed and committed at checkpoints 147, 151, 155, and
  158. No graph, topology, M0--M7, Bayesian, external-validation, or outcome workflow is rerun.
- `/Volumes/1TB/marklab/runs/science-crc-final-01-v4` remains the canonical bundle. A fresh
  read-only Mac mini audit rehashes all 1,905 declared artifacts with zero missing or mismatched
  files; 1,906 files include the manifest. Manifest and interpretation SHA-256 remain
  `1fe3dbeec3de956f2cdd6a9e67f8ccd983ae26301bfe44383fbb42a525b4f2b2` and
  `34f29d358deeac520facfd0cd644fc800461c2af049e89424d0d053ed35023ed`.
- Replay evidence remains intact without starting an external backend: all 64 patient categorical
  pair hits and all 16 patient witness-bottleneck hits are byte-identical to their misses, every
  corresponding ledger has one execution, and graph/topology ledgers retain exactly five/four
  prespecified variant executions per pattern. Thirteen focused final-sealer, graph/topology, and
  patient-witness tests pass.
- The scientific conclusion is unchanged. Descriptive local CRC H&E organization and one
  independent direction-level H&E association are supported; a stable transferable
  molecular-class spatial fingerprint is not established. The corrected cross-g sensitivities at
  checkpoint 174 are supplemental null/nonincremental evidence and do not alter the frozen fusion
  result or require an expensive scientific rerun.

## Type-specific inhomogeneous categorical cross-g checkpoint 176 — 2026-08-29

- Added `inhomogeneous_categorical_cross_pair_correlation`, its strict durable node, and `marklab
  project inhomogeneous-categorical-cross-pair-correlation`. The declared source and target levels
  each retain a separate existing Gaussian leave-one-out event/fixed-grid intensity artifact.
  Directed Epanechnikov contributions use the product of source/target inverse intensities and the
  eligible-source inverse-intensity denominator under standard-border `r+h` eligibility.
- The null independently samples complete source and target location patterns from their fixed
  type-specific grids while conditioning on both observed counts. Source/target seed namespaces,
  typed categorical/source/window/config/runtime identity, ERL, project/store/ledger/recovery,
  exact-float result bytes, and point/probe/intensity/pair/null/memory ceilings are explicit.
- An independent 20-by-20 Gaussian/grid calculation agrees on both type intensities, inverse-
  intensity kernel sum, center denominator, and normalized cross-g. Fewer than two rows per role
  and one-byte-short retained memory fail before output. Direct/durable tests pass 3/3 and the
  fresh-process CLI miss/backend-disabled hit is byte-identical with one ledger row.
- The prespecified real diagnostic reuses checkpoint 157's hash-frozen 512 cells and exact window,
  Neoplastic-to-Inflammatory direction, 20-micrometre radius, 50-micrometre intensity bandwidth,
  10-micrometre pair bandwidth, 16-by-16 grid, 19 simulations, seed 20260829, and fixed 1e-12
  minimum intensity. The identity-final binary hashes to
  `8f5640960a44a29f07b3589e2b12ddef7209a1d90706848cad805b987dd8b5c5`. Execution stops in 9.68
  seconds at 22,986,752-byte RSS because a role-local event intensity is 1.10276e-32 per square
  micrometre. No result publishes and the ledger remains empty; no parameter or subset is changed.
- Seventeen affected regression tests, two seed tests, warning-denied affected Clippy, package
  no-default compilation, strict affected docs, formatting, and whitespace checks pass. The first
  no-default attempt correctly exposes a CLI-only encoder annotation and passes after the encoder
  remains available to the feature-independent durable node. No broad workspace/Nextest loop runs.

## Durable typed scalar-semivariogram checkpoint 177 — 2026-08-29

- Added `ScalarVariogramAnalysisNode` and `marklab project scalar-variogram` around the existing
  observed scalar semivariogram and exact histologic-compartment whole-value random-labeling ERL
  family. The statistic, lag-bin semantics, null, and direct result types are unchanged. A strict
  version-one exact-float document binds the continuous mark/status, declared input, physical
  frame/window, lag bins, pair-plan digest, conditioning, permutations, seed, alpha, and work limits;
  result-format 0.3 is untouched.
- The node performs a conservative retained-memory preflight before registering inputs. Hits decode
  and validate typed input/window/config/result invariants without recomputing pairs or null curves.
  The existing categorical adapter gains an opt-in morphology-predicted `nucleus_area_um2` column
  only for this command; default categorical pair/cross-g column order, provenance, and bytes remain
  unchanged.
- The four-point hand curve remains 19/3, 24.5, and 32 across its three bins. The focused test now
  proves a too-small memory failure, durable miss, reopened hit, exact output equality, one ledger
  execution, and seed invalidation. A fresh CLI process produces a miss followed by a backend-
  disabled byte-identical hit and one ledger row.
- Real CRC execution is unavailable without fabricating a derived mark: the frozen v54 typed cells
  header contains coordinates, hard class, and compartment but no `nucleus_area_um2`. The admitted
  upstream CellViT JSON has pixel-space contours; no repository-owned contour-to-physical-area
  scale/provenance contract currently promotes those contours as this continuous mark.
- Five affected Moran/Geary/variogram and categorical CLI tests pass. Warning-denied affected
  Clippy, package no-default compilation, strict affected docs, and formatting pass. No broad
  workspace/Nextest loop, backend, benchmark, fuzzing, packaging, dependency, or publication run.

## Real CellViT contour-area scalar variogram checkpoint 178 — 2026-08-29

- Added a separate `prepare-scalar-variogram` path to the existing frozen CRC categorical worker.
  It verifies each selected CellViT source row, computes the absolute shoelace area of its admitted
  pixel-space predicted contour, multiplies by the recorded `base_mpp` squared, and writes the
  morphology-predicted `nucleus_area_um2` mark with exact source-payload, scale, unit, derivation,
  patient, slide, cell, and window provenance. Contours are limited to 3--4096 finite vertices and
  derived areas must be finite and positive.
- The default categorical `prepare` path remains separate and unchanged. A fresh real preparation
  over all eight patients, 16 slides, and 512 cells per slide is byte-identical to the sealed v54
  preparation tree. The scalar preparation admits all 16 frozen slides; the first fixed pattern has
  areas 2.3456--94.2629 square micrometres at 0.2501 micrometres per source pixel.
- The prespecified first slide runs `marklab project scalar-variogram` at 0/25/50/100-micrometre
  lag edges with histologic-compartment conditioning, 19 whole-value permutations, seed 20260829,
  130,816 pair visits, and a 64-MiB ceiling. Semivariances are 162.4852, 167.4144, and 166.4334
  square-micrometre-squared; all are inside the simultaneous envelopes and global p is 1.0. This
  null-compatible one-specimen result is retained as capacity evidence, not patient inference.
- The miss completes in 11.13 seconds at 22,757,376-byte maximum RSS. A fresh process with external
  backend execution disabled returns a byte-identical hit in 6.07 seconds; the result SHA-256 is
  `bd2dc7a02be8bba4e82d0558a2146736cf578c5ce9dec44f7e6d7a0ce16cbcc7` and the ledger has one row.
  The 90-file bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v58-scalar-variogram-work`; its run manifest
  hashes to `c42f0284a8dd5e0f9bbefb097acc07976ce09665e11c636a6b16ffc5c0b59585`, and an independent
  complete rehash reports zero errors.
- The two-test Python workflow suite, worker/test byte compilation, current CLI binary build,
  focused whitespace checks, remote compatibility comparison, result comparison, ledger check, and
  bundle rehash pass. No Rust production source changed after checkpoint 177, so affected Rust
  Clippy/no-default/docs are unchanged and not repeated. No broad workspace/Nextest loader loop,
  tuning, benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history
  rewrite runs.

## Patient CellViT scalar-variogram checkpoint 179 — 2026-08-29

- Extended the immediate scalar caller across the frozen eight-patient/16-slide design through
  `execute-scalar-variogram` and `summarize-scalar-variogram`. Six bounded processes run one strict
  durable `marklab project scalar-variogram` per slide at fixed 0/25/50/100-micrometre lag edges,
  with 19 histologic-compartment whole-value permutations, seed 20260829, exact pair/permutation
  work, 64-MiB memory, and 120-second per-process ceilings. The patient summary decodes exact-float
  results, nests both slides inside each patient, fits preprocessing inside held-out folds, permutes
  complete patient labels, bootstraps complete patients, and applies step-down Max-T across all
  three admitted endpoints.
- The 16 misses complete in 28.09 seconds at 22,790,144-byte parent maximum RSS. A fresh
  backend-disabled pass completes 16 byte-identical hits in 24.76 seconds at 24,100,864 bytes;
  every project ledger remains one row. The production oracle covers exact-float decoding, all 16
  misses/hits, replay bytes, nested patient reduction, held-out inference, whole-patient nulls, and
  Max-T while preserving all categorical pair/cross-g behavior.
- Nested-slide patient-rank stability has median 0.881, minimum 0.810, and q10 0.810. Scalar-only
  leave-one-patient-out balanced accuracy is 0.75 with exact whole-patient p=0.143. M0/M3 balanced
  accuracy is 0.625; adding the scalar block remains 0.625, an increment of 0.0 with whole-patient
  interval [-0.375, 0.375], while retrieval changes from 0.375 to 0.5. Every patient endpoint has
  step-down adjusted p=0.099. Thirteen of 16 within-slide curves are wholly inside their discrete
  envelopes; minimum global p is 0.05. Slides are diagnostics, not independent population units.
- The prespecified positive-increment gate fails, so the stable scalar block is not promoted or
  fused. No endpoint, lag, threshold, subset, null, or model is tuned. Acquisition-site leakage
  evaluation remains unavailable because the admitted CPTAC manifest has no site identity.
- The 199-file remote bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v59-patient-scalar-variogram-work`;
  its run-manifest SHA-256 is
  `ef5b8120251dfd4db34a365d573695fec3181e5f707e6edaebf9615a46dea5a4`, and complete rehash reports
  zero errors. The two-test Python workflow suite, source compilation, focused whitespace, real
  miss/hit/ledger comparison, summary checks, and bundle rehash pass. No Rust source changed, so
  checkpoint 177's affected Rust gates remain current. No broad workspace/Nextest loader loop,
  benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history rewrite
  runs.

## Durable raw-vector semivariogram checkpoint 180 — 2026-08-29

- Added `VectorSemivariogramProjectNode` and `marklab project vector-semivariogram` around the
  existing complete-vector weighted squared-Euclidean semivariogram. The direct mathematical owner
  and `marklab bayes vector-semivariogram` remain unchanged. Exact raw input, physical bins,
  optional pair weights, native binary/runtime, adapter revision, point/dimension/pair ceilings,
  memory budget, execution policy, and result schema now own the durable cache identity.
- The strict typed version-one result decoder validates source hashes, object/dimension/feature/bin
  identities, exact pair count, weight mode, finite nonnegative weights/semivariances, eligibility,
  and final-bin closure without recomputing the curve. Sources are hashed before and after bounded
  parsing; raw plus parsed retained-memory admission occurs before the durable project opens.
- The independent weighted hand/rotation oracle remains 2 and 8/3. The new behavior test first
  fails on the absent project command, then proves a one-short point ceiling publishes no output or
  project, miss, fresh backend-disabled hit, byte equality, one ledger row, and exact parity with
  the existing direct CLI.
- The admitted real 512-cell by 1,280-dimension CellViT input runs at 130,816 unordered pairs and a
  64-MiB ceiling. The identity-final miss completes in 11.88 seconds at 39,436,288-byte maximum
  RSS; the hit takes 6.95 seconds at 36,044,800 bytes. Both and the direct CLI hash to
  `399580c6c8dcc8cacd87191090bd779ac80f2a2c89d1cbf9b0571a80ebf13155`, with one ledger row.
  Semivariances are 4428.87, 5181.00, 5885.69, and 6326.83 over 0--25, 25--50, 50--100, and
  100--200 micrometres. This is one-slide capacity evidence, not patient inference.
- The 15-file bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v61-durable-raw-vector-variogram-final`;
  its run manifest hashes to `d9690cb0f716dac2b2032f614c5808d805bff16264d29813cee631046ad7df6f`
  and complete rehash has zero errors. Both focused CLI tests, warning-denied affected Clippy,
  package no-default compilation without warnings, affected-file formatting, source/direct/replay
  comparisons, and whitespace checks pass. No broad workspace/Nextest loop, benchmark, fuzzing,
  packaging, dependency, push, publication, deployment, or history rewrite runs.

## Durable patient M4 raw-vector checkpoint 181 — 2026-08-29

- Added `marklab_tcga_crc_m4_durable.py` as the immediate patient caller for the checkpoint-180
  project command. It validates the sealed admission, patient/input identities and hashes, fixed
  physical bins, 32-cell/1,280-dimension per-patient bounds, exact unordered-pair ceilings, frozen
  references, six-process ceiling, per-process timeout, replay bytes, and one-row ledgers.
- The first 169-project miss pass completes every durable project in 251.61 seconds but refuses to
  seal because byte comparison finds a current-runtime semivariance difference. Exact inspection
  finds zero structural differences and only 27 values across 25 patients at exactly one IEEE-754
  ULP. The compatibility boundary therefore requires identical structure/nonfloat values and at
  most one finite-float ULP; two ULPs fail. Completed misses resume cross-process without statistic
  execution in 1.11 seconds and publish the truthful execution manifest.
- A fresh backend-disabled pass returns 169/169 hits in 235.29 seconds at 24,264,704-byte parent
  maximum RSS. Every hit is byte-identical to its current-runtime miss, compatible with its frozen
  reference, and backed by one ledger execution. The existing sealed M4 patient stability,
  held-out, site-aware, and cohort conclusions are unchanged.
- The 1,196-file bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v62-patient-durable-raw-vector-final`;
  run-manifest SHA-256 is `28e3a37dceebeed20f42519935f43fd770cdbb8a2a205a82dc5f8716ff5ac762`
  and complete rehash reports zero errors. Three focused patient-input/durable tests, source
  compilation, resume/replay/reference checks, and whitespace checks pass. No Rust source changes
  follow checkpoint 180, so its affected Rust gates remain current. No broad workspace/Nextest
  loop, benchmark, fuzzing, packaging, dependency, push, publication, deployment, or history
  rewrite runs.

## Scalar/vector durable stabilization checkpoint 182 — 2026-08-29

- Stabilized checkpoints 178--181 as one four-workflow sequence: real contour-area preparation and
  scalar capacity, patient scalar inference, durable raw-vector project execution, and durable
  169-patient M4 replay. Direct review retains the distinct scalar/vector codecs and scientific
  gates, the shared existing Bayes vector CSV adapter, and no speculative registry or runner.
- `cargo +1.96.0 fmt --all --check`, warning-denied workspace all-target/all-feature Clippy,
  workspace no-default compilation, all-feature workspace doctests, and strict warning-denied
  all-feature workspace docs pass once without findings.
- The documented macOS Nextest/full-integration loader loop is not retried. Focused behavior,
  real-data miss/hit, reference-compatibility, remote rehash, and patient-unit evidence remains at
  checkpoints 178--181. No feature matrix, benchmark, fuzzing, packaging, dependency, push,
  publication, deployment, or history rewrite runs.

## Durable projected embedding-variogram checkpoint 183 — 2026-08-29

- Added `marklab project projected-embedding-variograms` around the existing pinned SciPy 1.18.1
  workflow. The durable node reuses the direct parser, training-only PCA, within-stratum complete-
  vector null, component-by-scale Max-T result codec, scheduler, store, ledger, and recovery path.
  Exact input/bin/lock/worker, Python/SciPy, request, controls, timeout, work, and runtime identities
  participate in the cache key; a typed hit is validated without starting Python.
- The direct and durable behavior tests pass. A fresh process with backend execution disabled
  returns a byte-identical hit with one ledger row. The admitted 3,000-row/16-dimensional projected
  CellViT input runs four components, 20 permutations, seed 20260826, and at most 2,000,000 pair
  visits. Miss, hit, and direct outputs all hash to
  `d75d753ce9b94306a293f8e72b9add1316e8ac6eea06a4fe6c11aaaaa83ace00`.
- The current result is structurally and numerically identical to the frozen result for the PCA
  artifact, bins, and curves; only the exact current lock/input/request identities differ. The
  18-file bundle at
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v63-durable-projected-variograms-final`
  rehashes without error; run-manifest SHA-256 is
  `f4104e6b37aa706e29a0232f6e695f35e0a050b5e9619ac01b1d93bd26cc666b`.
- Affected formatting, both focused integration tests, warning-denied affected Clippy, package
  no-default compilation, and whitespace checks pass. Checkpoint 182 remains the latest broad
  non-loader stabilization and is not repeated. This closes the interrupted durability increment;
  subsequent work is restricted by the SCIENCE-CRC-FINAL-01 override.

## Full-tissue gastric CellViT interim checkpoint 184 — 2026-08-30

- Added one narrow standalone gastric adapter over the active seven-slide CellViT run. It admits
  only four explicitly completed slides, verifies every graph/cell row correspondence, scans every
  1,280-dimensional embedding for finite full-slide mean/SD, and retains all five hard-class
  counts. The four slides contain 742,771 cells across all 5,593 available tissue patches and three
  patient lanes, including one completed pre/post pair. The fifth slide continues independently.
- Every slide is partitioned label-blind into four nearest-anchor tissue fields using four
  farthest-point patch anchors. Up to 2,000 deterministic cells per field feed the existing durable
  45/50/55-micrometre sparse scattering workflows; all 80 misses have one-row ledgers and all 80
  fresh backend-disabled hits are byte-identical. Coordinate-perturbation median/q10 field-rank
  stability is 1.0/1.0 in every slide. Cell-subsample and nearby-scale medians are 1.0 throughout,
  with isolated feature minima down to 0.4 retained rather than hidden.
- The first 2,000-cell topology design and a 512-cell/64-landmark correction truthfully fail the
  existing 1-MiB durable output ceiling in 11 and nine dense-field requests. The retained final
  approximation uses at most 512 witnesses and 32 farthest-point landmarks. All 64 corrected
  misses and 64 backend-disabled hits pass with one ledger and byte equality. Topology remains
  mixed: legacy coordinate thresholds pass only 8/16 fields; cell-subsample q10 ranges 0.052--0.840
  and nearby-scale q10 0.689--1.0. No interim promotion gate is applied.
- Full-slide versus spatially balanced sample embedding-mean cosine is 0.996, 0.992, 0.845, and
  0.997; the lower GS-26-1340 value is retained as spatial heterogeneity/sampling sensitivity.
  The one complete patient's pre/post cosine is 0.958 for composition, 0.968 for nonspatial
  embeddings, 0.970 for graph, and 0.994 for coarse topology. These are descriptive paired values,
  not treatment effects or population inference.
- The remote provisional summary is
  `/Volumes/1TB/marklab/runs/gastric-he-cellvit-interim-summary-v1`. Its 11-artifact manifest and
  summary hash to `6f834c48af8c691588990ee098ac771165cbe3e92a277d67ee8e040b93152475`
  and `a0f248f7802aad529347b917a0ac693b8602c726c8f59ff3937ec459371d8f21`;
  independent rehash reports zero mismatches. Five focused tests and source compilation pass. No
  broad workspace gate, completed CRC workflow, active CellViT process, or external publication is
  touched.

## Dense durable witness and 64-landmark gastric checkpoint 185 — 2026-08-30

- `marklab project witness-persistence` and `witness-persistence-stability` now admit at most
  32 MiB of typed durable output, consistently across the project ledger, inline artifact store,
  and scheduler. The change is local to these two immediate callers; their existing 16-MiB bounded
  worker streams, input/work/simplex/timeout controls, exact backend identity, codecs, transaction
  behavior, and result-format 0.3 remain unchanged.
- Red-first dense regressions produce 19,689,026-byte direct witness and 21,364,731-byte aggregate
  stability results against the former 1,048,576-byte ceiling. Both then complete a miss followed
  by a fresh backend-disabled byte-identical hit with exactly one ledger execution. The two full
  affected integration targets pass 2/2 each.
- The four completed full-tissue gastric slides are regenerated from their exact source artifacts
  at 512 witnesses and 64 farthest-point landmarks. All 64 topology misses complete in 118.09
  seconds at 80,035,840-byte parent maximum RSS; the fresh backend-disabled pass returns 64/64
  byte-identical hits in 144.30 seconds at 62,095,360 bytes, with one ledger row per project. The
  largest published result is 3,553,790 bytes. A separate fresh graph root completes 80 misses and
  80 disabled hits with the same replay/ledger guarantees.
- The retained checkpoint-184 v2 inventory is corrected from its mid-run count to 56 completed and
  eight failed 64-landmark requests. Both superseded 53/11 and 56/8 attempts remain named in the
  new summary. The 64-landmark result stays mixed rather than being promoted: cell-subsample q10 is
  0.078--1.0, nearby-scale q10 is 0.787--1.0, and only 4/16 fields pass the legacy coordinate
  thresholds. The one paired patient's topology cosine is 0.985; no population, treatment, or
  significance claim is added.
- The 11-artifact canonical provisional summary is
  `/Volumes/1TB/marklab/runs/gastric-he-cellvit-interim-summary-v3`. Manifest and summary SHA-256 are
  `ca151f925b44f2d0e05152c57e00fe164754bad39986a1913e40f836dde4fec8` and
  `0202d80853bcdabf54c62776a97f03e0dbc7358ef113dcf75bdcde2e0817de1d`; remote independent rehash
  reports zero mismatches. Six Python tests, both Rust integration targets, affected warning-denied
  Clippy, package no-default compilation, affected formatting, and whitespace checks pass. No broad
  workspace/Nextest loop runs.

## Durable embedding cross-covariance checkpoint 186 — 2026-08-30

- Added `marklab project embedding-cross-covariance-by-distance` around the existing IC-0083
  globally centred, symmetrized distance-bin matrix statistic. The caller binds exact input/bin,
  point/dimension/pair/matrix-work/memory, native runtime, executable, scheduler, implementation,
  and result-schema identities before using the existing project ledger, scheduler, artifact store,
  recovery, and transaction path. One-short point, dimension, and matrix-work limits fail before
  project creation or output publication.
- The durable artifact uses Marklab's existing exact-float codec so scheduler normalization retains
  every matrix bit while the user-facing ordinary JSON remains byte-identical to the direct CLI.
  Its strict decoder checks all identities, finite global means, exact bins and closure, matrix
  dimensions/symmetry, typed empty states, pair accounting, and trace/Frobenius consistency. No
  result-format, statistic, dependency, plugin, or generalized embedding framework changes.
- The full admitted 3,000-row/16-component source would require 1,151,616,000 matrix operations and
  is rejected by the fixed 250-million ceiling. The pre-existing held-out test split supplies a
  deterministic 600-row/six-patient view requiring 179,700 pair visits, 46,003,200 matrix
  operations, and 1,024 stored elements. No pair within the fixed 200-micrometre bins crosses a
  patient or permutation stratum.
- The real miss completes in 6.21 seconds at 28,114,944-byte maximum RSS. A fresh process with
  external execution disabled returns a hit in 6.39 seconds at 25,526,272 bytes; direct, miss, and
  hit all hash to `8a3960be892f2423cfa4c295235105c973bec11d0007d881f233ecddb8edab8a`,
  and the ledger remains one row. This is descriptive split-level capacity evidence: cells/pairs
  are not patient replicates and no population, molecular, recurrence, causal, clinical, or
  significance claim is made.
- The remote bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v64-durable-cross-covariance-final`.
  Its run manifest and checksum file hash to
  `0f7535245c1cb10ee7c7407f57b5c267baeb033df95c119784407ed0f1a154c5` and
  `d5010254e058da0f250d8ab3def0de6769a8f41c2ea9a11bed104102809ee5e1`; complete remote rehash
  passes. Both focused integration tests, warning-denied affected Clippy, package no-default
  compilation, affected formatting, and whitespace checks pass. No broad workspace/Nextest loop
  runs.

## Durable embedding kernel mark-correlation checkpoint 187 — 2026-08-30

- Added `marklab project kernel-mark-correlation` around the existing IC-0085 training-frozen
  linear/cosine/RBF/Laplacian kernel statistic. Exact parsed input/bin bytes, kernel/tolerance,
  point/dimension/pair/memory controls, implementation, native runtime/executable, scheduler, and
  result schema now own the durable identity. The existing analytic statistic and direct CLI are
  unchanged.
- Admission now reproduces all core row, feature, split, and physical-bin invariants before the
  durable project opens. The memory ceiling conservatively includes the radial training-pair
  distance vector, capacity growth, and stable-sort scratch. Artifact identities are derived from
  the exact bytes parsed and then checked against the live sources, closing the preparation ABA
  gap. One-short source, radial-memory, point, dimension, pair-work, invalid-row, and invalid-bin
  cases publish neither project nor result.
- The admitted 3,000-row/30-patient/16-component CellViT table requires 3,597,600 pair visits and
  57,561,600 component operations. Training-only RBF scale is 0.8774239814749609. Normalized
  similarity decreases across 0--25/25--50/50--100/100--200 micrometres in training
  (1.418/1.308/1.238/1.174), validation (1.342/1.244/1.195/1.143), and test
  (1.400/1.302/1.233/1.163); all assigned nearby-bin pairs are within patient.
- The durable miss takes 9.12 seconds at 46,481,408-byte maximum RSS. A separate
  backend-disabled process returns a hit in 6.60 seconds at 22,282,240 bytes. Direct, miss, and hit
  are byte-identical at SHA-256
  `aa9986d6440f84c752272fb80b4d82f84e221362ad8e84d9a2ad4fd80b0f2115`, with one ledger row.
  This is descriptive split-level capacity evidence, not patient-population or molecular-class
  inference.
- The sealed bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v65-durable-kernel-mark-final`.
  Run-manifest and checksum-manifest SHA-256 are
  `3edba3f64c6884a43b6f5d7c6f654fb0086fdce9c0041ecdf4a6e9dd1adb4848` and
  `92a7248200ec4a7c345a7acd23e697fbc7ae7ba265f6485e2fdd24c16891e21a`; complete remote rehash
  passes. Both focused tests pass on the mini snapshot; the analytic direct test and isolated
  executable durable test pass in the current checkout. Package no-default compilation and
  affected formatting/whitespace checks pass. Warning-denied affected Clippy is not green because
  it stops on the concurrent refactor's existing `clippy::ptr_arg` finding at
  `src/bin/marklab/bayes/embedding_spatial.rs:190`; that user-owned file is not changed. No broad
  workspace/Nextest loop runs.

## Durable embedding spatial-envelope checkpoint 188 — 2026-08-30

- Corrected IC-0086 so both the observed curve and complete-vector random-label null use only
  unordered pairs inside each declared permutation stratum. The prior all-row pair plan could mix
  specimens despite stratified donor shuffling; an exact two-stratum oracle now fixes the expected
  pair/work counts.
- Added `marklab project embedding-spatial-dependence-envelope` with exact input/bin/configuration,
  native runtime/executable, semantic graph, work, retained-memory, and typed-result identity. The
  direct and project integrations pass together; a fresh backend-disabled hit is byte-identical and
  leaves one ledger row.
- The admitted 960-cell/30-patient/39-ROI input has 13,378 within-ROI pairs. Direct, miss, and hit
  hash to `04654b89e7b02f11e636cfea3561297d2be8d70c8371c2b86e90261a2723cd59`.
  The sealed v66 run manifest hashes to
  `3edf71a1e4afb7a5144e17956643b195fcc25d77e38c1d030591e5213db19546`.
  Its `1/21` result is pooled conditional capacity evidence, not patient-population significance.

## Durable graph-signal checkpoint 189 — 2026-08-30

- Added separate project commands for existing graph Dirichlet energy, stratified graph-smoothness
  permutation, and local embedding roughness. Each admits exact node/edge semantics and resource
  limits before project creation, binds exact graph/source/config/native-runtime identities, and
  validates typed cached output without adding a graph runner or registry.
- All three focused project targets pass 1/1. The real 960-cell/39-ROI/2,729-edge caller completes
  direct-byte-identical misses and backend-disabled hits with one ledger each. Dirichlet energy is
  `3.2489033417571664`; smoothness is `1/21`; local roughness retains 57 islands. These remain
  descriptive or within-ROI diagnostics and are not promoted into the sealed patient fingerprint.

## Durable patch-summary and patient-inference checkpoint 190 — 2026-08-30

- Added bounded project commands for the existing multiscale embedding kernel, pinned-SciPy
  cell-patch complementarity, patient MMD, energy distance, complete-family Max-T, and ordered
  hierarchical Max-T. They preserve direct result bytes, patient/block/fold/family/null semantics,
  exact source and serialized-path identity, SciPy 1.18.1/Python 3.12 lock/worker identity where
  applicable, native runtime identity, and explicit work/output/memory ceilings.
- One combined focused command runs all six new integration targets; all pass 1/1. It initially
  exposed the concurrent CLI refactor's omitted shared 16-MiB input constant; restoring that exact
  pre-existing bound closes 34 unresolved imports without changing any adapter limit. The run emits
  only unrelated in-progress refactor warnings and no warning-denied claim is made.
- Five admitted real callers run in parallel and replay disabled byte-identically with one ledger
  each. Multiscale, complementarity, MMD, energy, and Max-T result SHA-256 values are respectively
  `b4feb099d5240be7f94187dc1d6517f0d06a46bf369febfdb4b58ba3e8766713`,
  `ea801ca7330d7534f8fd067aab18c8f3ed624b932726d6dc5869fbd339f5b69f`,
  `eee927ecf7477f6e7c4886bb836bcced1e53f6c09fcafb2d59acb2a17d41f184`,
  `2d621a46c4a644f5aa8eaea7e705d1cb820317bea55dccdd5c34dd220a622089`,
  and `a505c0828af896815ab4885e062e0f7d936507cc41e2d2bc8fe2fd77c659aa42`.
- The canonical 91-entry bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v67-durable-graph-patch-patient-final`.
  Run-manifest and checksum SHA-256 are
  `30afac24097f54348f97237df33af6a0092acfafac3f7d752a5ee078e3afbd6f` and
  `f6db914260d75b206517568113968ac7f19046a68d844a016b4cb26198a79c0a`; remote complete rehash
  passes. The real results remain null-compatible. Hierarchical Max-T lacks a prespecified real
  family map, and independent H-Optimus tensors/links remain exact unavailable lanes.
- No workspace-wide/Nextest gate, all-feature matrix, Clippy, no-default check, docs build,
  benchmark, fuzzing, packaging, dependency audit, push, publication, deployment, or history
  rewrite runs at this sprint checkpoint.

## Durable adjusted whole-cluster checkpoint 191 — 2026-08-30

- Added `marklab project cohort-cluster-covariate-permutation` around the existing typed
  equal-weight cluster-summary Freedman–Lane workflow. Exact input/path, group, alternative,
  permutation/seed, patient/cluster/covariate/cell-work/OLS-work/memory controls, native executable,
  scheduler, result-schema, and implementation identities own the durable cache key. Five
  one-short resource cases fail before project creation; direct, miss, and disabled-hit bytes match
  with one ledger row.
- The admitted TCGA caller contains 167 patients across 22 explicit tissue-source sites. COAD/READ
  project identity is constant within all sites (19/3), stage ordinal is the nuisance covariate,
  and the outcome is the already selected M2 relative-L endpoint at 50 micrometres. The adjusted
  COAD-minus-READ effect is 0.14124092013604375 (SE 0.11094896415426271), studentized statistic
  1.2730260369052504, and two-sided 999-permutation p-value 0.168. This is null-compatible
  observational evidence, not a causal, molecular-class, or clinical claim.
- Direct/miss/hit results hash to
  `ca528e2d2d8a1db33cf6ef8b431ab1e9575a7bc50922c015378a5d31a65e5892`. The sealed 13-file
  bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v68-durable-adjusted-cluster-final`;
  run-manifest and checksum-manifest SHA-256 values are
  `937ca6bb65f54fb9f46fc3b21bba483ea75a3b8fc09c4ce8f390cbb04885a467` and
  `032e96ba49b874d6c561e3f11103fd46b8caeeec02a3ea5d68bd5b7114b514c7`, and remote complete
  rehash passes.
- The focused direct/project integrations pass together. No workspace-wide/Nextest gate, broad
  feature matrix, benchmark, fuzzing, package, dependency, push, publication, deployment, or
  history rewrite runs.

## Durable patient-first hierarchical-bootstrap checkpoint 192 — 2026-08-30

- Added `marklab project cohort-hierarchical-bootstrap` around the existing typed
  patient-then-nested-specimen nearest-rank percentile workflow. Exact input path/bytes,
  replicate/seed/alpha, patient/specimen/draw/memory, native executable, scheduler,
  implementation, and result-schema identities own replay. Patient, specimen, and worst-case draw
  one-short cases fail before project creation; direct, miss, and disabled-hit bytes match with one
  ledger row.
- The real caller retains the already selected M2 coordinate-only relative-L endpoint at 50
  micrometres for 627 specimens nested under 169 TCGA patients. With 999 deterministic replicates,
  the observed specimen-row mean is 0.12502070998960083 and the 95% percentile interval is
  [0.09433228205974918, 0.1593254062734362]. This is descriptive patient-first nested-sampling
  evidence, not a group, causal, molecular, equivalence, or clinical result.
- Direct/miss/hit results hash to
  `e7e24fdd6c54824a00b1b63088a45a5c92745c0b9aeb9743f3806d8708a5a76e`. The sealed 13-file
  bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v69-durable-hierarchical-bootstrap-final`;
  run-manifest and checksum-manifest SHA-256 values are
  `b5b8c8589b06106fd466aabba91e295accf7d750392f6af3f6672f22a8f911c7` and
  `37b72249c42bb7075bada35cdb62cf1cd5487f66d64d4b7d26c98d73a54f3851`, and remote complete
  rehash passes.
- The focused direct/project integrations pass together. No workspace-wide/Nextest gate, broad
  feature matrix, benchmark, fuzzing, package, dependency, push, publication, deployment, or
  history rewrite runs.

## Durable adjusted multisite and non-loader stabilization checkpoint 193 — 2026-08-30

- Added `marklab project cohort-multisite-covariate-contrast` around the existing patient-unit,
  within-site fixed-nuisance OLS and fixed/REML pooling workflow. Exact source/path, groups, model,
  alpha, patient/site/covariate/cell-work/OLS-work/memory, native executable, scheduler,
  implementation, and result-schema identities own replay. Five one-short limits fail before
  project creation; direct, miss, and disabled-hit bytes match with one ledger row.
- The real TCGA caller admits 126 patients across nine tissue-source sites with at least two MSI and
  two MSS patients, varying stage, full-rank intercept-plus-stage-plus-group designs, and positive
  residual degrees of freedom. The stage-adjusted random-effects REML MSI-minus-MSS effect for the
  existing 50-micrometre M2 coordinate-L endpoint is -0.010404190594993534 (SE
  0.06627998333104117), with 95% CI [-0.14031057081974932, 0.11950218962976225], prediction
  interval [-0.25027919798195153, 0.22947081679196443], tau-squared 0.010585653379058362, and Q
  11.791406035753479. This is null-compatible observational evidence.
- Direct/miss/hit results hash to
  `195b24f804ccb21ac57d45cb66e8e528a507d51b7f2aed7a3c968d16a6cd6a71`. The sealed 13-file
  bundle is
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v70-durable-adjusted-multisite-final`;
  run-manifest and checksum-manifest SHA-256 values are
  `83d25b7b6a09b734ebffe0ddd3ba02883008a56fc11fe8b4dc07018ef1489fca` and
  `439548fc790045dd0efafd2e8280ff61e72c1552f0920c1e8aa9940eada4442c`, and remote complete
  rehash passes.
- The six checkpoint-191--193 direct/project integrations pass together. Workspace formatting,
  warning-denied all-target/all-feature Clippy, workspace no-default compilation, all-feature
  workspace doctests, and strict warning-denied all-feature workspace docs pass once. The
  documented macOS Nextest/full-integration loader loop is not run. No benchmark, fuzzing,
  packaging, dependency, push, publication, deployment, or history rewrite runs.

## CellViT uncertainty semantics and durable support mixing checkpoint 194 — 2026-08-31

- Added exact physical-radius conservative pair bounds for sources that genuinely expose one
  top-1 class posterior plus the native class count. The typed result retains coordinatewise lower
  and upper pair mass, exact mark/status/window/graph/configuration identities, hard work/memory
  ceilings, and durable miss/hit replay. The admitted CellViT source is deliberately rejected:
  row 462 has `type_prob=0.00395256915`, below the five-class top-1 minimum 0.2.
- Frozen CellViT postprocessor inspection establishes the actual semantic: `type_prob` is the
  fraction of pixels inside a nucleus instance assigned the selected non-background type. The
  adapter and real MarkTable fixture now identify it as `cellvit_winning_type_pixel_support`.
  A threshold-free multiclass sensitivity workflow retains the hard directed matrix and weights
  every fixed-label pair by source-times-target pixel support; it never changes labels or claims a
  class posterior.
- The corrected pinned adapter revalidates 366 slides, 178 patients, and 1,542,389 cells into
  `/Volumes/1TB/marklab/runs/results-cellvit-categorical-v71-support-semantics-inputs`. Coordinate
  and window SHA-256 values are `83e60b742fe33ac504a89e8c57e824611cfe5a6c8bbbdfd50f2056c6b82b32dc`
  and `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4`.
  On 49,510 directed 50-micrometre visits, support-weighted mass is 44,887.23086408087 and
  retention is 0.9066295872365354. Fresh processes report miss then hit, identical result SHA-256
  `513c96af7cea718b156704d520bcec22fc55a6250441db3d41e360599049bd62`, and one execution.
- Cell/patch/region/slide Arrow and Parquet read/write/publish/scan/verify paths already exist and
  were not duplicated. One inspected Schürch bundle has a 1,074-by-1,280 float32 NPY and matching
  frozen CSV header, but its bundle manifest lacks the model, environment/lock, license, and
  converter/provenance records required by the canonical promotion graph. That lane remains
  unavailable at this exact identity boundary; no graph was fabricated or weakened.

## Patch interchange, scalable motifs, and durable spatial Bayes checkpoint 195 — 2026-08-31

- Canonical patch-embedding Arrow artifacts can now be materialized through the existing store
  integrity envelope from exact expected-patch, support, and provenance bindings. The reconstructed
  typed table has the same logical digest and produces the identical patch-overlap dispersion as
  the original in-memory table; a false support logical identity is rejected.
- Typed triangle motifs now enumerate only canonical forward wedges, retain hard work/output/token
  bounds, and expose a compact direct/durable summary without changing the detailed version-one
  result. The admitted v71 CPTAC graph has 2,000 cells, 24,755 radius edges, and 194,608 wedge
  checks. Its Neoplastic/Inflammatory/Connective result has 1,744 observed triangles, permutation
  counts 5,950--8,825, and upper-tail `p=1.0`; this one-slide diagnostic is null-compatible and not
  a patient replicate. Compact direct/miss/hit output is 835 bytes at SHA-256
  `50813197ec7e28a320cc672b1db58df7662582c5ae634c409005a6915e2a56d8`, with one ledger row.
- The existing pinned-PyMC 6.3.0 one-dimensional Matérn spatially varying coefficient family now
  runs through `marklab project` with exact source/path, predictor, prior, sampling, lock, worker,
  backend, runtime, and result identity. The focused short run is honestly nonconverged but replays
  byte-identically without a second sampler execution; the existing long direct oracle remains
  complete. Independent sampler runs are not claimed bitwise identical.
- The sealed CRC inputs still lack distinct ROI-within-slide identities and a prespecified
  field-level SVC outcome/predictor design; all admitted M7 rows are 217 slides under 105 patients.
  No ROI/cohort/hurdle/anisotropic family or real CRC posterior is invented. Focused integrations,
  55 embedding, 7 graph, and 60 Bayesian library tests, affected warning-denied Clippy, clean
  no-default checks, strict affected docs, formatting, and whitespace checks pass. No workspace
  Nextest/full-loader loop or other broad gate runs.

## Advanced workflow durability checkpoint 196 — 2026-08-31

- Added six concrete typed project paths over existing production algorithms: exact-window PyMC
  IPP physical spatial PPC, growth-front SMC-ABC, linear-Gaussian Kalman/Joseph/RTS, physical
  cuboid 3-D K/L, randomized binary interference, and scalar Gaussian EIG. Each binds the exact
  source, complete controls, implementation/native runtime, typed result codec, and existing
  algorithm bounds through the current scheduler, artifact store, ledger, and recovery owners.
- Fresh processes prove miss then hit with one ledger execution for every workflow. The PyMC PPC
  hit succeeds with `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`, proving no second worker start.
  The direct physical PPC, logistic-growth recovery, scalar Kalman fractions, two-point 3-D
  translation K, six-state exposure probability, and analytic `ln(2)/2` EIG oracles remain green.
- No new scientific claim is made. SMC-ABC remains synthetic and uncalibrated biologically;
  longitudinal and 3-D workflows lack registered repeated/volumetric CRC specimens; causal
  interference lacks admitted treatment identification; Gaussian EIG lacks prospective outcomes.
  These are production execution paths, not evidence that the broader BAY-PP, WS-73, DIM-01, or
  CAU-01 families are complete.
- The 12 named direct/project integrations, four affected library test/doc suites, affected
  warning-denied Clippy, affected no-default compilation, affected-file Rustfmt, and diff
  whitespace checks pass. The documented workspace Nextest/full-loader loop and unrelated broad
  gates are not run.

## General voxel-window and registered-serial 3-D checkpoint 197 — 2026-08-31

- Added exact physical finite voxel-union 3-D windows with occupied volume, exposed-face surface,
  six-connected components, enclosed cavities, canonical logical identity, half-open membership,
  exposed-face border distance, and exact box-intersection translation overlap. None, border, and
  translation K/L retain hard grid, pair, pair-radius, boundary-face, overlap, and memory ceilings.
- Added a deterministic registered-serial path over the canonical coordinate registry. Exact
  patient/specimen/timepoint/site/volume/section identities, consecutive order, section thickness,
  physical Z, affine placements, points, and explicit missing planes are retained. The synthetic
  three-plane oracle embeds two observed cells around one missing plane and preserves two
  disconnected occupied voxels; distorted sections fail with the exact transform-draw blocker.
- Both workflows run directly and through `marklab project`; fresh processes prove miss then
  backend-disabled hit with one ledger execution. This does not claim real 3-D or longitudinal
  evidence. Transform-posterior propagation into K/L, watertight mesh/tetrahedral windows, and an
  admitted registered cohort remain open.
- Six spatial3d package tests plus docs, four focused CLI integrations, affected warning-denied
  Clippy, no-default compilation, formatting, and whitespace checks pass. No broad workspace
  Nextest/full-loader loop or unrelated gate runs.

## Paired registered longitudinal K-change checkpoint 198 — 2026-08-31

- Added one paired registered-volume workflow over the existing exact serial voxel K path. It
  requires the same patient/lesion/site, distinct specimen and timepoint identities, ordered biopsy
  days, a bounded treatment/exposure interval, cross-time registration and deformation-posterior
  identities, aligned deformation-only K-change draws, a negative-control curve, and an independent
  change curve. Radii, correction, and anisotropic metric must match exactly.
- Results retain both full timepoint analyses and report observed K change, deformation-only and
  registration-adjusted 95% intervals, negative-control compatibility, independent-direction
  agreement, and their prespecified conjunction. The positive oracle changes from K 0 to K 3 under
  symmetric ±0.25 registration draws; the null oracle retains zero change and is not promoted.
- Direct and durable CLI paths pass; a fresh process proves miss then backend-disabled hit, exact
  output bytes, and one ledger row. The parent registered-serial direct and durable paths also pass
  after the preparation refactor. Affected warning-denied CLI Clippy, formatting, and whitespace
  checks pass.
- This remains synthetic correctness evidence. No real registered longitudinal cohort, deformation
  estimator, probabilistic cell correspondence, causal treatment effect, or same-cell claim is
  added. No broad workspace Nextest/full-loader loop or unrelated gate runs.

## Frontier point-process and admission checkpoint 199 — 2026-08-31

- Added direct and durable NumPyro execution for bounded simulation-based calibration of the
  existing replicated arbitrary-window multitype inferred-kernel LGCP. Five fixed positive, null,
  weak-identification, boundary, and misspecified scenarios simulate and refit the exact
  quadrature counting measure. Rank/coverage summaries retain intensity, group contrast,
  patient/pattern hierarchy scales, field amplitude/length, and one identified latent node;
  posterior prediction retains count, mark-proportion, cross-type-enrichment, and clustering
  checks. Scenario-specific seeds and simulation, iteration, PPC, event, memory, output, and
  timeout ceilings are part of the durable identity. The model still has conditionally independent
  type fields; no unestimated cross-type covariance is invented.
- Added one normalized joint patient-replicated location/conditional-mark model over the existing
  provenance-paired tables. The lexicographically first of two patterns per patient trains and the
  second evaluates. Exact-window Poisson total-location intensity and a fixed-radius conditional
  categorical mark graph share only an identified patient ecology effect; independently refitted
  location-only and mark-only baselines use the same holdout. The direct and project paths retain
  typed inputs/results, backend/environment/worker/configuration identity, patient as the
  population unit, resource ceilings, predictive diagnostics, and backend-disabled durable replay.
- The real authorized CPTAC caller contains eight patients, sixteen patterns, 666 quadrature rows,
  and 8,192 mark rows. Two fresh project processes return byte-identical SHA-256
  `8a475670cbb4e8fd856f106dcbc1169e6eae0d9fe874bebacb12eb40b58989fd` with one ledger
  execution. The fit is truthfully `nonconverged`: maximum R-hat is 1.074975, minimum bulk ESS is
  33.084, and 59 maximum-tree-depth hits remain. Joint minus separate-baseline held-out log
  predictive density is 2.94 with SD 201.63 and interval [-404.77, 377.62]; all three predictive
  tail probabilities are zero. It is diagnostic-only, null/uncertain, and was not retuned or
  promoted. The shared effect does not establish communication, causality, or a biological latent
  mechanism.
- A bounded read-only audit of the sealed CRC outcome bundle finds no identified real causal
  caller. TCGA/CPTAC contain no admitted treatment exposure. Schürch has postoperative therapy for
  7 patients, no therapy for 13, and missing status for 15, but no treatment time. Stanford has 33
  treated, 12 none, and 7 unknown, but no treatment time and no event/censor indicator for
  `DaysSurvival`. No lane jointly supplies an exposure definition/time, follow-up origin and
  censoring, measured adjustment set, interference design, negative control, sensitivity inputs,
  and positivity support. CAU-01/WS-82 therefore remain unavailable for real evidence rather than
  fabricating a treatment effect.
- The same sealed artifacts declare no authorized ROI/stain/landmark/field/replicate/sequencing
  decision candidate, feasible set, budget, utility/loss, outcome model, operational constraints,
  or outcome-blind historical evaluation split. ACT-01/WS-84 therefore remain unavailable for a
  real prospective outcome; the existing Gaussian EIG and synthetic allocation laboratories are
  not relabeled as operational benefit.
- The four focused direct/project integrations pass serially 4/4, Python worker bytecode
  compilation passes, and affected warning-denied CLI Clippy passes. One preceding Cargo command
  selected two nonexistent descriptive target names and exited before running tests; the corrected
  canonical target command passed. Workspace formatting and final diff whitespace checks pass. No
  workspace Nextest/full-loader loop or unrelated broad gate runs.

## Frontier fitted-field and local-multivariate checkpoint 200 — 2026-08-31

- Added four concrete mathematical workflows. A replicated arbitrary-window multitype LGCP now
  estimates an LKJ-Cholesky cross-type correlation with positive marginal field scales. A separate
  patient-replicated model jointly fits exact-window total-location intensity and fold-frozen
  projected embeddings, with identified loadings and posterior-mixture patient held-out scoring
  against a nonspatial patient baseline. Both run directly and durably through the pinned NumPyro
  environment and replay with backend execution disabled.
- Added native local multivariate Moran inference over complete feature rows. Global feature
  standardization and fixed physical-radius row-standardized weights are prespecified; complete
  vectors move only within declared strata, and one maximum-absolute randomization family controls
  every reported location. The four-point/two-feature oracle is exactly `[1,0,0,1]`; direct and
  project paths pass, and a fresh process returns a byte-identical hit with one ledger row. This is
  a within-specimen field diagnostic, not cell-level population inference.
- Replaced the rectangular-only SPDE boundary for one immediate projected-embedding field caller.
  Rust validates the canonical polygon/multipolygon topology, area, and digest; the pinned SciPy
  worker builds deterministic boundary-refined meshes, reports discretization-area error, assembles
  piecewise-linear mass/stiffness and alpha-two precision matrices, performs bounded barycentric
  projection, and fits one fixed-hyperparameter spatial factor. A holed-square oracle preserves the
  hole and constant-mass integral; a disconnected-window oracle produces two components with no
  cross-component precision entries. Direct and durable paths pass, and the disabled-backend hit
  proves replay without a second worker.
- Focused direct/project integrations, three prior advanced-Bayes regressions, Python bytecode
  compilation, 27/27 cohort-library tests, and 296/296 root-library tests pass; 21 explicit manual
  performance/calibration tests remain ignored. Root/cohort no-default checks, strict affected docs,
  warning-denied affected CLI Clippy, formatting, and whitespace checks pass. The documented macOS
  Nextest/full-integration loader loop is not run.
- These are mathematical correctness and durable-execution milestones, not new real biological
  findings. Real promotion of the joint embedding and adaptive SPDE fits still requires a
  provenance-complete projected-embedding table paired to the same admitted exact window and
  region/cell coordinate identity; no causal, communication, significance, or patient-population
  claim is inferred from the synthetic planted or FEM oracles.

## Real CellViT local-field and adaptive-SPDE checkpoint 201 — 2026-08-31

- The canonical per-slide projected CellViT schema now feeds local multivariate inference directly.
  It requires exact `cell_id,x_um,y_um,cellvit_pc_*` columns, derives one slide stratum from every
  `slide:cell` identity, rejects mixed slides, preserves all source values, and binds the schema in
  the cache identity. A 100-row decimal regression exposed and fixed one-ULP direct/project JSON
  drift by publishing the direct result through the same typed durable codec.
- Read-only Mac-mini admission uses projected source SHA-256
  `efd2a3fa9855692ed8ebd1d54faabbea282b3a07f38973f3d578dae4bc8d2940` and exact-window SHA-256
  `9ba8102b98f5e4f42dc4c19d9fd341b0acb6d6ecf671997ce709187312c61fb4` for slide
  `ea6df59d-5240-4927-b19d-d44590`. All 2,318 rows and 16 projected components are finite and inside
  the admitted 12-component patch-union window. At the already declared 200-micrometre scale, the
  graph has 609,838 directed edges and 60,373,962 permutation-edge evaluations. The global
  maximum-absolute randomization p-value is 0.01 and 455 local rows have adjusted p at most 0.05.
  Direct/miss/hit files are byte-identical at SHA-256
  `6ddacfa402b5405d184a1c35612f7641029f37b74f6818fccff7874d5ef4724c`; the ledger has one row.
- The real adaptive-SPDE lane uses 128 exact-window cells selected solely by domain-separated
  SHA-256 Cell-ID rank, 16 fixed projected components, kappa 0.01, tau 1, noise SD 0.1, and final
  request SHA-256 `8ea3b8fd9a675dfad690b26a863076ae7afca93e585a0939d96e7f1899f97059`.
  The 286-vertex/391-triangle mesh preserves all 12 components, has relative area error
  0.0006782, positive minimum precision eigenvalue `1.4752e-5`, and exact projection row sums. The
  fitted one-factor RMSE is 0.1237 and its x-correlation is only 0.180. The warmed solver terminates
  under SciPy's relative-objective criterion after 4,452 iterations and retains maximum gradient
  0.00613 as a numerical limitation.
- Adaptive durable miss/hit files are byte-identical at SHA-256
  `e368a74a2b1c6f105f2f60ac44fe63633c88cdc9c200617b210c99238876c3b1` with one ledger row and a
  backend-disabled hit. Direct and project payloads have no nonnumeric differences and only two
  float-codec differences, bounded by `6.62e-24`. These results are single-slide local-field
  diagnostics, not patient/cohort inference or evidence of biological communication, molecular
  recurrence, causality, or clinical utility.
- The four focused local/adaptive CLI targets pass 7/7; Python bytecode compilation, affected
  warning-denied CLI Clippy, formatting, and whitespace checks pass. No broad workspace loader
  loop, benchmark, fuzzing, packaging, dependency audit, push, publication, or deployment runs.

## Patient-nested CellViT fields and adaptive-SPDE checkpoint 202 — 2026-08-31

- Added one complete patient-nested field reducer with direct and durable CLI paths. Equal-weight
  specimens remain inside patients, preprocessing is refit inside every leave-one-patient-out fold,
  and whole-patient step-down Max-T owns the endpoint family. Incomplete specimen vectors,
  nonfinite/degenerate inputs, and patient/specimen/endpoint/permutation-work/memory overruns fail
  before inference.
- The admitted CPTAC design contains 15 patients, 35 slides, and 16,525 retained projected-CellViT
  cells after coordinate-only radius-isolate removal. All 35 baseline local-field projects and 140
  prespecified cell-subsample, one-micrometre-jitter, and nearby-scale projects replay with backend
  execution disabled and one ledger row. Patient median relative RMS change is 0.0084 for jitter,
  0.0248/0.0586 at 180/220 micrometres, and 0.0601 under 80% cell subsampling.
- The complete-case 14-patient incremental analysis refits standardization and PCA inside every
  patient-held-out fold. Technical plus composition balanced accuracy is 0.864 with exact
  whole-patient `p=0.0522`; adding raw nonspatial CellViT produces 0.394, and adding the local field
  changes balanced accuracy and retrieval by exactly zero with bootstrap interval `[0,0]`. The
  field is therefore not fused or promoted.
- The arbitrary-window SPDE caller uses a uniform, label-blind 128-cell sample per slide and the
  backend's fixed 10,000-iteration hard ceiling after the 5,000-iteration design proved
  capacity-inadequate. All 35 fits converge and replay byte-identically. The three sign-invariant
  patient endpoints have step-down adjusted `p=0.997` each and held-out balanced accuracy 0.0417;
  this is retained as a null/negative result. Summary SHA-256 values are
  `669d6f1d378b9f46695ea7ac8fb0f83ce121c7271e90c599a49db1ce4aa4f369`,
  `9c9fdac9253bc2a7b4d10d360e751e508a926e0365e927eebada6f8dcdf400b8`,
  `1bc9603c2648c3b520b7cd6bc28d7e99d0d66dacaa9c67dfce038e68c32b300d`, and
  `7de3bda9664d763f71462846a5b452923865d3a142546b4ad7e7aee999198111`.
- The Mac mini lacks the repository's pinned Python 3.12 environment and worker checkout. Exact
  prepared requests were therefore executed in this checkout's pinned environment and the
  verified 35-slide durable projects were sealed beside the source manifests on the 1 TB drive;
  no backend identity check was weakened. The adaptive result codec now deterministically repairs
  a real adjacent-float JSON oscillation and carries implementation identity v2.
- Focused direct/durable integrations pass 11/11, the adaptive codec regression passes, the cohort
  library passes 27/27, and the Python caller passes 8/8. Affected warning-denied Clippy,
  no-default checks, strict cohort docs, affected-file Rustfmt, and diff whitespace checks pass.
  The documented workspace Nextest/full-loader loop and unrelated broad gates are not run.

## Exact multiplicity calibration and cross-fitted intensity checkpoint 203 — 2026-08-31

- Added an exhaustive whole-patient calibration workflow for complete endpoint families. Every
  fixed-size group assignment is treated in turn as observed and as part of the exact null;
  independently recomputed Welch statistics calibrate single-step, step-down, and contiguous
  ordered-family step-down gatekeeping. The eight-row/three-endpoint oracle has 70 assignments and
  exact global-null familywise rejection counts 6, 6, and 4 at alpha 0.1. Its exact 63,910
  comparison ceiling and one-short failure include triangular step-down/gatekeeping work.
- `marklab cohort max-t-calibration` and `marklab project max-t-calibration` retain the complete
  patient table, family sizes, alpha, group size, assignment/work/memory limits, runtime, and result
  identity. A fresh project process returns a byte-identical hit with one ledger row; this is
  finite-design method calibration, not scientific effect or power evidence.
- The existing exact-window Gaussian K/L owner now optionally cross-fits intensity by balanced
  stable-Cell-ID rank folds. Event rows use only complementary folds, each fold estimator is scaled
  by total/training count, and the fixed null grid is the heldout-size-weighted mean of the same
  estimators. Fold count, Cell IDs, training counts, intensity/grid artifacts, configuration, and
  work/memory are cache-bound; the prior leave-one-out constructor and bytes remain unchanged.
- On the frozen 512-cell exact-window caller, five-fold training counts are 409/410 and 20,588,683
  intensity evaluations complete in 12.68 seconds at 21,381,120-byte maximum RSS. Miss and disabled
  hit hash to `e8d55f38f6cdcdb0753b4d94df6626e53ade662dcda10aa65a7c636717b2741e`
  with one ledger row. L-minus-r is -18.630, +14.259, and +20.986 micrometres; global `p=0.10` is
  diagnostic and unpromoted. Bundle `SHA256SUMS` hashes to
  `158385bc79a9345e3d4c740f995f90bde5c7f6eca4bc1d78995a2bc729d68563`.
- Type-specific cross-g reuses the identical fold contract independently inside each declared
  role. Its real Neoplastic-to-Inflammatory caller still fails before publication: row-zero heldout
  intensity `1.3784497481671735e-32` is below the prespecified `1e-12` floor. The ledger remains
  empty; blocker and bundle hashes are
  `96e728c824c1c52bfbedba61f4e9bc8e3cdae6ba00b3ac2500048c1893012562` and
  `c5a9c8dd2e927bd1c58220418046ce5a647232757692d8cf2d1cc09e580bb8c5`.
- Eight focused direct/durable integrations pass 8/8 and two exact calibration unit tests pass.
  Warning-denied affected root-library/root-CLI/cohort Clippy, no-default root/cohort checks, strict
  affected docs, affected-file Rustfmt, and whitespace checks pass. Local R 4.5.2 lacks
  `spatstat.explore` and the Mac mini lacks R, so pinned external agreement remains unavailable.
  No workspace Nextest/full-loader loop or unrelated broad gate runs.

## Durable patient-level ordinal likelihood checkpoint 204 — 2026-08-31

- Added one strict patient-level proportional-odds likelihood through pinned PyMC 6.3.0/Python
  3.12. The typed contract requires unique complete patients, exactly two declared groups with at
  least four patients each, two through eight prespecified fully represented ordered levels,
  ordered cutpoints without a free intercept, finite positive prior scales, and explicit
  patient/level/iteration/output/process ceilings. Direct and project commands publish the same
  result contract; backend, environment lock, worker, request, source bytes, configuration, seed,
  executable, and runtime participate in durable identity.
- The invalid-level unit oracle and planted 24-patient direction oracle pass. A fresh durable
  project executes PyMC once, replays byte-identically in a second process with backend execution
  disabled, rejects a changed seed while the backend is disabled, and retains one ledger row.
- The real admitted TCGA CRC caller contains 126 unique patients, 78 MSS and 48 MSI, with stage
  codes 1--4 represented by 22/54/35/15 patients. The fit completes with maximum R-hat 1.00085,
  minimum bulk/tail ESS 1955/2353, minimum E-BFMI 0.972, no divergences, and no tree-depth hits.
  The MSI-minus-MSS common log-odds effect is -1.014 with 95% interval [-1.675,-0.361]. This is an
  association only, not progression, treatment, causal, or clinical evidence.
- The level-2 posterior-predictive tail probability is 0.00775, so the one-coefficient
  proportional-odds model is retained as materially misspecified despite computational
  convergence; no threshold, subset, prior, scale, or likelihood was retuned. Miss/hit result SHA
  is `219d1a9a7a46cb6ddbbefa259eebc7d8fd165b582163f0589c7aa6c1f5c4948b`, and the sealed
  `/Volumes/1TB/marklab/runs/results-cellvit-ordinal-stage-v75` `SHA256SUMS` digest is
  `28984eb998cfaa0bd995677c333b90ba84f39f7970edb6a9ecabe62ed1a0765d`.
- Focused unit/direct/durable tests, Python bytecode compilation, affected warning-denied Bayes and
  root-CLI Clippy, Bayes no-default check, strict Bayes docs, affected-file Rustfmt, and diff
  whitespace checks pass. No workspace Nextest/full-loader loop or unrelated broad gate runs.

## Durable patient-level hurdle likelihood checkpoint 205 — 2026-08-31

- Added one strict exposure-aware patient hurdle likelihood through pinned PyMC 6.3.0/Python 3.12.
  A Bernoulli-logit component owns hard-class presence, and a zero-truncated beta-binomial owns
  positive counts conditional on presence and total-cell exposure. Both groups must contain zeros
  and positives; cells remain patient measurements. Separate component intercepts/group effects,
  one shared concentration, typed results, complete diagnostics, exact backend/lock/worker/request/
  source/runtime identity, and patient/trial/iteration/predictive/output/process ceilings are
  retained through direct and durable commands.
- The first real posterior-predictive implementation correctly stopped at its 1,000-attempt
  rejection ceiling and produced no result or ledger row. It was replaced without changing the
  likelihood or sampling controls by exact conditional beta-binomial CDF inversion: at most one
  bounded quantile evaluation occurs per simulated present-patient draw, under a fixed five-million
  work ceiling.
- The real admitted CPTAC caller contains 105 patients, 81 MSS and 24 MSI, 886,751 total classified
  cells, and hard-Epithelial zero/positive counts of 14/67 and 5/19. The fit completes with maximum
  R-hat 1.00189, minimum bulk/tail ESS 1842/1367, minimum E-BFMI 0.859, no divergences, and no
  tree-depth hits. Presence group log odds are -0.134 with interval [-1.215,0.985]; conditional
  positive-abundance group log odds are -1.110 with interval [-3.828,0.475]. Both are retained as
  null/uncertain and do not establish biological absence, mechanism, causality, or clinical value.
- Real miss/hit result SHA is
  `06afde770318494e35e2a21b9c07323f78092df97f2394b7f21cace9fc734b90`; the second process
  disabled external execution and the ledger has one row. The sealed
  `/Volumes/1TB/marklab/runs/results-cellvit-hurdle-epithelial-v76` checksum-file SHA is
  `d5ca64e3b0881232af5c873dd9b36f83ebca826744bacdc4142b6d752d13a0a9`.
- Focused unit/direct/durable tests, Python bytecode compilation, affected warning-denied Bayes and
  root-CLI Clippy, Bayes no-default check, strict Bayes docs, affected-file Rustfmt, and diff
  whitespace checks pass. No workspace Nextest/full-loader loop or unrelated broad gate runs.

## Durable ordinal site-varying hierarchy checkpoint 206 — 2026-08-31

- Added a patient-level proportional-odds hierarchy with sum-zero noncentered site intercepts and
  site deviations from one global molecular-group slope. Admission requires 8--64 exact sites and
  at least two patients from both groups per site; source, site, level, backend, lock, worker,
  configuration, seed, runtime, and resource identity are durable.
- Unit, direct, and project oracles pass. The durable workflow executes once, replays byte-identical
  with external backend execution disabled, rejects changed-seed execution, and retains one ledger
  row. Python compilation, affected warning-denied Clippy, Bayes no-default/docs, Rustfmt, and
  whitespace checks pass.
- The real TCGA caller has 126 patients across nine sites, every site containing MSI and MSS. The
  global log-odds effect is -1.027 with interval [-1.714,-0.343]; site-intercept SD is 0.277
  [0.010,0.833], and site-slope SD is 0.342 [0.014,1.055]. Both variance components remain broadly
  uncertain near zero. Level-2 PPC tail probability remains 0.00367, so site heterogeneity does not
  repair the proportional-odds misspecification.
- Miss/hit SHA is `4b97efb61a6b14c0d0df1b59c1391e95019283694082cb2fae89379c7b8e71f5`; sealed v77
  checksum-file SHA is `c348445c93d1a15f965df5be6e9de126bc9b97aebd07ee3bc82dc813028e5f19`.
  ROI-within-slide, crossed, and cohort effects retain exact identity blockers.

## Valid non-proportional ordinal checkpoint 207 — 2026-08-31

- Added separate strictly ordered MSI/MSS cutpoint vectors with sum-zero site intercepts. Threshold
  effects are reference-minus-comparison cutpoint differences, so both cumulative distributions
  remain monotone by construction. Direct/durable commands retain exact typed and backend identity.
- Unit/direct/durable oracles pass, including opposing threshold effects, site-confounding rejection,
  changed-seed invalidation, and byte-identical backend-disabled replay. Affected Clippy,
  no-default, docs, Python compilation, formatting, and whitespace checks pass.
- On 126 TCGA patients, threshold effects after stages 1/2/3 are 0.136 [-0.784,1.117], -1.485
  [-2.274,-0.688], and -3.125 [-5.613,-1.211]. Category PPC tails are 0.995/0.957/0.999/0.800,
  resolving the proportional models' profile failure in-sample. Result SHA is
  `bc90ec2f52cc68028674fbffd6ea40e0c446f614b5910ef3ae7f53d915148a4e`; sealed checksum SHA
  is `30ef33216abe776f9e037a4f27e535d3ab5c922ad0298f9c45579aff1be63a3b`.
- Site-held-out log-score comparison remains required before promotion; no causal or clinical claim
  follows from the improved PPC.

## Site-held-out ordinal comparison checkpoint 208 — 2026-09-01

- Added deterministic leave-one-entire-site-out patient scoring for the proportional and valid
  non-proportional ordinal models. Every fold fits only its training sites, scores every patient in
  the held-out site, and reports category log score under explicit smoothing, optimizer, row-work,
  and memory ceilings. Direct and native durable project commands share the typed result.
- Unit, direct, and durable behavior tests pass. The durable workflow executes once, restores the
  same typed bytes in a fresh process, and retains one ledger row. Warning-denied affected Bayes and
  root-CLI Clippy plus whitespace checks pass; no broad loader loop runs.
- On 126 TCGA patients across nine sites, mean held-out log score is -1.28046 for proportional odds
  and -1.22850 for non-proportional odds, an improvement of 0.05196 nats per patient. Six sites
  improve and three worsen, so the result supports the non-proportional distribution overall but
  does not establish universal site transportability.
- Miss/hit result SHA is `b5508994b386ebf6ceb9c9ffdedf0e7aafa237cff28ca5139c8ff8f9b3ca930c`;
  ledger SHA is `bd7c45024ebf0a8c803ad2c9f9782170af510bc688f9f83f5f661ebc95d5ad34`;
  sealed v79 checksum-file SHA is `0bc6097c9c5cacdcabf4be97ac7a386b74a0e880b8944b76abbf3777e4213ce2`.

## Crossed/nested Gaussian hierarchy checkpoint 209 — 2026-09-01

- Added one fixed pinned-PyMC hierarchy with distinct cohort intercept/slope, patient
  intercept/slope, slide, ROI, crossed batch, and residual variance components. Sum-zero transforms
  retain an identified fixed intercept/slope; patients remain the statistical unit.
- Admission requires replicated ROI-within-slide-within-patient identity, three supported cohorts,
  within-patient/cohort exposure variation, and a genuinely crossed patient/batch graph. Confounded
  or unreplicated designs stop before backend execution. Direct and durable project commands retain
  the exact typed request/result and backend/environment/worker/runtime identity.
- The planted 12-patient/24-slide/48-ROI/4-batch/3-cohort, 192-observation fit completes with zero
  divergences and zero depth hits under the strict diagnostic policy. A second bounded project
  fixture executes once, replays byte-identically with backend execution disabled, rejects a changed
  seed, and retains one ledger row.
- No admitted real table currently supplies all distinct ROI-within-slide, crossed batch, and
  compatible multi-cohort outcome identities. Those real estimates remain unavailable with that
  exact blocker; no biological transport or variance claim is fabricated.

## Nonstationary anisotropic adaptive-SPDE checkpoint 210 — 2026-09-01

- Added one exact-window α=2 FEM specialization with prespecified piecewise local κ/τ, rotated
  determinant-one anisotropy tensors, and region-specific interior refinement. Exact polygon holes
  and disconnected pieces remain owned by the existing window contract; circular parameter regions
  must be nonoverlapping and refinement is bounded before triangulation.
- The hand oracle recovers the 45-degree ratio-four tensor `[[2.125,1.875],[1.875,2.125]]`, proves
  smaller median triangles inside the declared focus, positive symmetric precision, exact hole
  retention, and a converged penalized factor. Direct and durable commands pass; the fresh process
  restores exact typed bytes without a second SciPy execution and retains one ledger row.
- Existing stationary boundary-adaptive tests remain green 3/3 after the shared pinned worker
  change. Warning-denied affected root-CLI Clippy, Python compilation, affected Rustfmt, and
  whitespace checks pass.
- The admitted real CellViT local/adaptive field blocks previously failed their prespecified
  incremental or group gates. This mathematical capability therefore remains synthetic-oracle
  validated and caller-dependent; no real tissue anisotropy/nonstationarity claim is made.

## Robust multitype inhomogeneous edge checkpoint 211 — 2026-09-01

- The existing type-specific Gaussian intensity/fixed-grid null workflow now supports exact polygon
  translation and exact visible-arc isotropic correction in addition to its byte-compatible standard
  border default. Every observed and null pair carries the selected correction; correction work and
  limits are typed, validated on replay, and included in configuration identity.
- Independent square-window hand oracles recover translation normalization by overlap area 300 and
  isotropic normalization by visible fractions one and window area 400. The combined user-facing
  project target executes/replays both corrections with one ledger row each; focused standard and
  cross-fitted regressions remain green.
- Pinned external agreement is still unavailable: local R 4.5.2 lacks `spatstat.explore`, and the
  Mac mini has no admitted R environment. This is an exact backend blocker, not native mathematical
  incompleteness, and no `spatstat` agreement is claimed.

## Durable declared-margin inference checkpoint 212 — 2026-09-01

- Added native durable project paths for the existing patient-level Student-t TOST equivalence and
  directional noninferiority analyses. Exact source bytes, declared margins, direction, alpha,
  rationale, native runtime, implementation, and output schema participate in cache identity.
- Direct analytical-oracle tests remain green. A fresh process executes each analysis once and a
  second process restores exact typed bytes with one ledger row; replay validates identity, bounds,
  decisions, and finite results without recomputing the statistic.
- These paths do not supply scientific or clinical margins. Real equivalence/noninferiority claims
  remain unavailable until an admitted caller provides protocol-fixed margins, rationale, endpoint,
  sign convention, and adequate independent patient observations.

## Robust joint location–embedding and catalog-admission checkpoint 213 — 2026-09-01

- Extended the existing fitted patient-replicated joint location–embedding model with one explicit
  finite-variance Student-t residual option. The same fixed degrees of freedom are used by the
  spatial joint fit, separately refitted nonspatial comparator, held-out posterior-mixture log
  density, and posterior prediction. Gaussian remains the default; family and degrees of freedom
  are prespecified and cache-bound, never selected from held-out performance.
- A closed-form two-dimensional Student-t density oracle passes at `1e-15` absolute tolerance.
  Invalid degrees of freedom at 2, nonfinite, or above 100 stop before backend execution. Legacy
  Gaussian direct/durable behavior remains green, while a robust fresh process executes once and a
  backend-disabled second process restores byte-identical output with one ledger row.
- Read-only Mac-mini admission finds location-table SHA-256
  `c862ce9790274f1f099f040716de864b5185cc3084004ccef7693376a95f791f` with 16 patterns and
  projected-table SHA-256 `3fe55c5b1e0e57395f64957956d527009c01440d1a4da828c973c790312377a2`
  with 65 slides. Only two patterns intersect, both belonging to one patient, so no real
  patient/group joint fit is identified. The 15-patient/35-slide local-field manifest supplies
  projected embeddings and windows but no matching exact location quadrature artifact. Real
  promotion remains blocked rather than reconstructing correspondence or treating slides as
  patients.
- Existing multitype point-process SBC already supplies prior-rank and posterior-predictive
  implementation calibration. Real biological SBI calibration remains unavailable because no
  admitted CRC caller defines a biological simulator parameter/observable correspondence,
  independent calibration target, or outcome-blind validation set. This is a data/model-validity
  blocker, not another generic SBC implementation gap.

## Real patient-replicated joint location–embedding checkpoint 214 — 2026-09-01

- Added one narrow real-data adapter over the already admitted 15-patient/35-slide CPTAC projected
  CellViT field manifest. It selects three MSI and three MSS patients by fixed SHA-256 patient rank,
  then two slides per patient by a separate fixed SHA-256 rank; no outcome, fit, effect, or held-out
  score participates. Every retained Cell ID, coordinate, and 16-component projected vector remains
  exact. The existing exact-window quadrature helper clips a 4x4 grid to each source MultiPolygon,
  and selected-cell counts conserve exactly between location and embedding tables.
- The sealed real table contains six patients, twelve slides, 5,446 projected cells, and 141
  positive quadrature nodes. Every patient has exactly two slides; MSI/MSS patient counts are 3/3;
  all point IDs are unique; pattern sets agree exactly; six patterns train and six evaluate. Input
  SHA-256 values are `448aa64c59d8c460dfea71ab24124c4d653ca7136f7b97121490132caeb896aa`
  for location and `4546740d42102e480b5f2be08b64ea3d76ebe7034bc99781aa18944a903ec322`
  for embeddings. The full v80 bundle checksum-file SHA is
  `1f9dd6645779be9b6db316a1166486e77687d7117a72f7edb7e15a01d7828591`.
- The prespecified two-factor, Student-t-df-5 real project fit completes once and replays in a fresh
  backend-disabled process with one ledger row. It is truthfully `nonconverged`: maximum R-hat is
  2.816, minimum bulk/tail ESS 2.56/5.13, minimum E-BFMI 0.0295, with one divergence and 326
  tree-depth hits. Count PPC is compatible (`0.81`), but embedding-RMSE PPC is `0.0`. Joint-minus-
  nonspatial held-out patient log density is 62.45 with interval [-226.98, 420.34], so no spatial
  improvement is established. Miss/hit SHA is
  `6e2c702fb631270d453a89b6d7d017801fad210ecd856346be34a2181b460131`; ledger SHA is
  `e56a8be3894f5ec77bf2416b500e66de629422a8c9e828020c3949953fe507fb`; result checksum-file
  SHA is `d3c988e2658494798f36f2fb2794cb2da7abe94e147bdda2b52c9d5dbc1c2b54`.
- The location process is the existing bounded, label-blind projected-cell selection, not whole-slide
  cell intensity. The fit is diagnostic-only and is not retuned, fused, interpreted as biological
  communication, or promoted as molecular discrimination.
