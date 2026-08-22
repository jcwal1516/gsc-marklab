# Refactor Decisions

Architectural and scientific decisions are append-only entries. Superseded decisions remain in this file with a link to the replacement.

## 2026-08-21 — Phase 0 opened from a clean linked worktree

- Context: The original checkout was on `branch/spatial-phenotype-recovery` with modified and untracked user work, while the master plan requires a clean start from `main`.
- Decision: Preserve the original checkout untouched and create `refactor/audit-remediation` as a linked worktree from `main` at `a642fbcdd80b5baf784cd633b707dc0283a24d11`.
- Consequences: All remediation changes and verification records live in `/Users/user/Bench/marklab-refactor`. The pre-existing branch and its dirty files are not part of this refactor and must not be modified, stashed, or discarded.
- Status: Accepted.

## 2026-08-21 — Phase 0 closed with test-only reproduction and benchmark scaffolding

- Context: Phase 0 required an untouched verification baseline, concrete critical-defect evidence, twelve minimal reproductions, and measured scaling before production remediation.
- Decision: Close Phase 0 at production baseline `a642fbc` with the test/benchmark harness committed through `ba7dd9f`. Keep desired-contract regressions ignored until their owning remediation phases, but require explicit `--ignored` runs to demonstrate the defects.
- Evidence: The exit Nextest run passed 249 tests with 23 expected skips; the all-feature Cargo suite passed with 167 library tests, 19 ignored library tests, 17/2 integration pass/ignore for engine spectrum, 15/1 for CLI, and 10/1 for WSI. Formatting, warnings-denied Clippy, doc tests, no-default-features, dependency policy, Machete, and fuzz builds passed. All ten benchmark groups exited 0 and recorded three-size medians plus peak RSS.
- Consequences: Production behavior is unchanged. The only production-file edits are guarded by `cfg(test)`: a multimodal engine-call counter and the private baseline harness. The matrix now distinguishes confirmed defects from pending static audit items.
- Remaining risk: `cargo audit` still reports three allowed warnings, including the newly reviewed `lru 0.18.1` unsoundness advisory. The public Aperio/OpenSlide test remains a scheduled external-fixture check. Profile timing is below the useful noise floor and needs a larger Phase 6 workload.
- Status: Accepted; Phase 1 may begin.

## 2026-08-21 — Phase 1 finite and statistical contracts

- Context: Median, mean, variance, extrema, and scalar permutation calculations were duplicated with incompatible missing-value, denominator, tie, and even-sample behavior. JSON serialization could also silently turn a non-finite value nested in an optional field into `null`.
- Decision: Use average-even medians; name reject-all-nonfinite and ignore-nonfinite paths separately; distinguish population from sample variance; represent undefined ratios as `Option`; and reject every non-finite floating-point value by traversing the serializable result before writing JSON or typed sidecars. Scalar permutation tests use inclusive ties, a plus-one correction, an explicit tail, and an explicit minimum permutation count.
- Consequences: The beta-binomial fallback coordinate split changed intentionally for even cell counts from the upper-middle coordinate to the arithmetic midpoint of the two middle coordinates. This corrects an inconsistent internal rule. Potentially undefined domain fields remain to be converted to typed states in Phase 2; the finite boundary prevents invalid persistence but is not a substitute for producer correctness.
- Status: Accepted.

## 2026-08-21 — Phase 1 deterministic seed namespaces

- Context: Spectrum, anisotropy, component, cross-interaction, enrichment, and curve-difference permutations used unrelated XOR constants. Their intended domains were not named or tested, and the common layer depended upward on the permutation module for SplitMix64.
- Decision: Make SplitMix64 a common primitive, derive seeds from a stable base seed plus a typed endpoint namespace and permutation index, and add a golden-value/domain-separation test. Migrate every endpoint touched in Phase 1. Feature-specific namespaces remain feature-gated so no-default builds stay warning-free.
- Consequences: Historical ad hoc permutation sequences changed. Exact prior sequences were not documented as a public compatibility contract; determinism across runs and thread counts remains the contract. Additional endpoints will be migrated when their owning workflows are touched.
- Status: Accepted.

## 2026-08-21 — Phase 1 closed

- Evidence: Commit `8508671` removes the duplicate numeric and scalar p-value implementations. `cargo +1.96.0 fmt --all --check`, warnings-denied all-target/all-feature Clippy, and `cargo +1.96.0 check --locked --no-default-features` passed. `cargo +1.96.0 nextest run --locked --all-features` passed 264/264 tests with 23 expected skips. All-feature documentation tests passed with zero doctests.
- Remaining scope: COR-04 producer semantics remain open: sparse enrichment can still construct infinity and zero-variance z-score sentinels, which the new boundary now refuses to persist. The Phase 2 typed-state change is required before enabling its ignored regressions.
- Status: Accepted; Phase 2 may begin.

## 2026-08-21 — COR-02 true rigid transform

- Context: `RegistrationTransform::Rigid` routed to a scale-plus-translation calculation with no rotation. Both the public engine and CLI repeated that incorrect dispatch, and the result identified the fitted model as `scale_translation`.
- Decision: Implement the closed-form orientation-preserving two-dimensional least-squares rigid fit, using normalized centered covariance terms for numerical range safety. The model estimates rotation and translation only, always has determinant +1, and cannot absorb scale or reflection. Delete the unused scale-plus-translation implementation rather than expose a transform with no present product requirement.
- Consequences: Existing configurations using `transform = "rigid"` now receive the algorithm that name promises. Their numerical results and `transform_type` metadata change from `scale_translation` to `rigid`; this is a documented correctness repair, not a silent schema-shape change. Affine behavior and metadata remain unchanged. The CLI still refits the transform for sidecars until ARCH-01/DUP-06, but both paths now use the same correct function.
- Evidence: Commit `53e2348`; 18 registration tests, a public-engine known-rotation test, and a CLI sidecar rotation test pass. Warnings-denied Clippy passed. The full all-feature Nextest run passed 275/275 with 22 expected skips after correcting an example-comment fixture interaction found by the first exit run.
- Status: Accepted; COR-02 closed.

## 2026-08-21 — COR-03 spectrum null sensitivity semantics

- Context: With stratification enabled, the configured stratified spectrum was primary and the confounding check reran the identical stratified spectrum. Homogeneous strata were treated as an ordinary p-value of one and preemptively labeled confounded.
- Decision: Keep the configured stratified null primary. Run one unstratified sensitivity over the already-resolved modes and observed powers. Define confounding as an evaluable unstratified low-k p-value below `family_wise_alpha` with an evaluable stratified low-k p-value at or above alpha. Both significant is robust-to-strata; no unstratified signal is not confounding. Mark-homogeneous strata are a degenerate stratified null, not a numeric result or an automatic confounding conclusion.
- Consequences: `primary_endpoint.null` now distinguishes stratified fixed-position random labeling. A degenerate primary spectrum is `InsufficientData` with `DegenerateSpatialStrataNull`; it no longer emits a misleading p-value of one. Internal summaries retain both p-global and low-k sensitivity values for tracing. Adding both summaries to the serialized result is deliberately staged for the version 0.3 schema work in Phase 5, so COR-03 remains open in the findings matrix only for that reporting requirement.
- Evidence: Commit `aecc554`; four rule tests, enabled distinct-null execution, homogeneous-strata, missing-strata, and the 20-test engine-spectrum suite pass. Warnings-denied Clippy and the no-default-features check pass.
- Status: Accepted for execution and conclusion semantics; serialized reporting pending Phase 5.

## 2026-08-21 — COR-04 typed enrichment statistics and result format 0.3

- Context: Sparse permutation nulls could have zero expected edges and zero or unestimable variance. The 0.2 model required numeric ratio and z-score fields, so production emitted infinity and a fabricated z-score of zero. JSON then silently converted infinity to `null`, breaking round trips and contradicting CSV/Parquet/report outputs.
- Decision: Make `enrichment_ratio` and `z_score` optional finite numbers and pair each with a typed unavailable reason. Zero expectation, zero variance, insufficient null samples, and defensive non-finite computation are distinct. Keep valid permutation p-values and adjusted q-values independent. Make CSV reason columns explicit, Parquet numeric/reason columns nullable, and reports print `undefined (<reason>)`.
- Consequences: This is a breaking public schema correction, so commit `4bf20e8` advances result documents to version 0.3 rather than silently altering 0.2. The initial `docs/result-format-0.3.md` and `docs/migration-0.2-to-0.3.md` record the change. The converter remains Phase 5 work; readers currently reject old documents. Shared result assembly now covers both stratified and unstratified enrichment rows, reducing duplicated semantics without introducing a strategy framework.
- Evidence: Both former ignored COR-04 regressions now pass, zero-variance coverage verifies its distinct reason, nullable Parquet artifact generation passes, warnings-denied Clippy and no-default-features pass, and all-feature Nextest passes 283/283 with 19 expected skips. The first full run exposed conditional serde field omission breaking CSV row widths; reason fields now serialize consistently and the rerun is green.
- Status: Accepted; COR-04 closed and result format 0.3 opened.

## 2026-08-21 — COR-05 typed curve availability

- Context: Empty pair-correlation bins were emitted as observed zeros, and pre/post or territory-profile comparison failures constructed `CurveTestResult { statistic: 0.0 }`. Consumers could not distinguish a measured zero from absent geometry or insufficient data.
- Decision: Preserve every configured pair-correlation bin but make its value optional. A bin with no contributing pairs has `count = 0`, `value = None`, no envelope bounds, and is excluded from global-envelope inference. Represent curve-test availability with a serde enum, make the statistic optional, and attach a diagnostic reason when the test is unavailable.
- Consequences: Result format 0.3 pair-correlation and curve-test fields are nullable. Parquet curve schemas mark values nullable, JSON emits explicit nulls, and downstream pre/post comparisons reject differing bin availability. Permutation curves may use an internal zero placeholder only for the identical geometry bins masked out by the eligibility vector; it never reaches a scientific result or rank calculation.
- Evidence: Commit `e7447c0`; the former COR-05 reproduction, spectrum, neighborhood-profile, pre/post, JSON, and output artifact tests pass. `cargo +1.96.0 fmt --check`, warnings-denied all-target/all-feature Clippy, and `cargo +1.96.0 check --locked --no-default-features` pass. The full all-feature Nextest run passes 284/284 with 18 expected skips.
- Status: Accepted; COR-05 closed.

## 2026-08-21 — COR-06 pre/post axis tolerance

- Context: Pre/post result documents reconstruct curve axes independently. Direct `f64` equality rejected mathematically identical decimals such as `0.1 + 0.2` and `0.3`, preventing valid difference and equivalence diagnostics.
- Decision: Compare corresponding finite spectrum modes and pair/cross-interaction bin edges with `|a-b| <= 1e-12 + 1e-12 * max(|a|, |b|)`. Continue to require identical axis lengths and matching bin availability. Non-finite or materially different values produce the typed unavailable curve result introduced by COR-05.
- Consequences: No result-schema field is added because current 0.3 documents do not carry a canonical axis identifier. The tolerance is explicit in `SPEC.md` and the result-format document and is deliberately much smaller than configured physical bin widths or mode spacings. A future structural axis definition may supersede numeric reconstruction but must preserve this compatibility behavior for independent documents.
- Evidence: Commit `e7f91ca`; the enabled reproduction covers spectrum, pair-correlation, and cross-interaction paths, and a material-difference test preserves rejection. The pre/post suite passes 8/8. Formatting, warnings-denied Clippy, no-default-features, and all-feature Nextest pass; Nextest ran 286/286 with 17 expected skips.
- Status: Accepted; COR-06 closed.

## 2026-08-21 — COR-07 independent input QC denominators

- Context: CSV and Parquet loaders copied the final retained-row fraction into `internal_control_valid_fraction`. Tumor validity, IHC validity, internal control, artifacts, and nonviable exclusions were not independently counted, so reported control performance changed when an unrelated filter excluded a row.
- Decision: Both adapters feed one `PatternBuildCounters` model. The denominator for every input QC fraction is all cells inside the tumor mask. Tumor-valid, IHC-valid, explicitly control-valid, artifact-excluded, nonviable-excluded, and final-retained numerators are independent; overlapping exclusions count in each applicable fraction. Optional fractions are absent only when their source state is unavailable. A present but blank control value is invalid, and zero in-mask denominators return an error.
- Consequences: Result format 0.3 adds nullable `valid_tumor_fraction` and `valid_ihc_fraction`; `valid_mask_fraction` is explicitly documented as final retained/in-mask. The obsolete `qc::ihc_validity::validity_fraction`, which mapped a zero denominator to numeric zero, was deleted. The broader CSV/Parquet row builder unification remains Phase 5 work, but filter and counter semantics no longer diverge.
- Evidence: Commit `6000cc8`; the enabled reproduction, combined-exclusion, tumor/IHC, internal-control, zero-denominator, CSV/Parquet parity, and result-propagation tests pass. Formatting, warnings-denied all-feature Clippy, CSV-only and Parquet-only checks, and no-default-features pass. The Parquet-only check retains three pre-existing dead-writer warnings. All-feature Nextest passes 289/289 with 16 expected skips.
- Status: Accepted; COR-07 closed.

## 2026-08-22 — MODEL-04 component mode execution contract

- Context: `Separate` and `Both` shared the same component-emission branch while the engine always calculated and presented pooled endpoints. `Pooled` returned an available empty component list, and `Auto` made an undocumented decision that was absent from the result.
- Decision: Resolve every request into a typed `Pooled`, `Separate`, or `Both` plan before endpoint execution. `Pooled` calculates pooled endpoints and makes component results NotApplicable. `Separate` calculates component spectra, skips pooled spectrum, pair-correlation, anisotropy, and multiscale execution, and marks every pooled endpoint NotApplicable. `Both` calculates both. `Auto` selects Both only when more than one component exists and the largest contains less than 0.80 of cells; otherwise it selects Pooled. Persist the requested mode, resolved mode, and reason in result format 0.3.
- Consequences: Separate results have no aggregate primary statistic; their primary endpoint fields are NotApplicable and component summaries own the available values. Missing component IDs under an explicit component mode are InsufficientData, not an available empty vector. `src/api/assembly.rs` now has explicit imports instead of its prior parent wildcard. Formal timing improvements from skipped pooled work remain for the performance phase.
- Evidence: Commit `b56cc60`; Pooled, Separate, Both, Auto-pooled, Auto-both, and reason assertions pass, as does the 20-test engine-spectrum suite. Phase exit formatting, warnings-denied Clippy, no-default-features, doc tests, and all-feature Nextest pass; Nextest ran 290/290 with 15 expected skips.
- Status: Accepted; MODEL-04 closed.

## 2026-08-22 — Phase 2 closed and Phase 3 opened

- Closure: True rigid registration, distinct confounding execution, finite enrichment states, typed curve unavailability, tolerant pre/post axes, independent QC denominators, and distinct component modes are implemented. Result-shape changes are documented under format 0.3. COR-03's persisted dual-null sensitivity field remains intentionally staged for Phase 5 and is still visible as in progress in the findings matrix.
- Verification: `cargo +1.96.0 fmt --check`, warnings-denied all-target/all-feature Clippy, `cargo +1.96.0 nextest run --locked --all-features` (290/290, 15 expected skips), `cargo +1.96.0 check --locked --no-default-features`, `cargo +1.96.0 test --locked --doc --all-features`, and the 20-test engine-spectrum integration suite all exit 0 on `b56cc60`.
- Phase 3 entry: Begin with a source-to-public-surface naming audit. No scientific rename will be made until the implementation and accepted technical meaning are recorded with evidence.
- Status: Accepted; Phase 3 may begin.

## 2026-08-22 — Phase 3 algorithm naming audit

- Context: Phase 3 requires names to describe implemented operations, including names not called out by the original static audit.
- Decision: Adopt the detailed evidence table in `docs/refactor/ALGORITHM_NAMING_AUDIT.md`. Follow the plan's default path for SCI-01 through SCI-03: rename the heuristic family to multiscale residual/scale-energy terms rather than implementing a wavelet or Gaussian-difference transform without a product requirement. For SCI-04, rename the one-window FFT to a Hann-tapered raster periodogram and correct its radial shell aggregation. For SCI-05, make the core interpretation neutral and retain MMR prose only in an MMR policy/report layer.
- Additional findings: Register SCI-06 (centered mark-pair covariance mislabeled pair correlation), SCI-07 (margin assessment mislabeled equivalence test), SCI-08 (beta posterior summaries mislabeled beta-binomial), and SCI-09 (pooled-bin diagnostic mislabeled difference test). Validation overstatement remains COR-01 and is scheduled for Phase 9; the current user-facing suite will be called a smoke check until then.
- Consequences: Phase 3 will make coordinated format-0.3, config, CLI/report, artifact, and documentation changes. Search-based negative assertions will ensure obsolete scientific terms do not survive in public surfaces. Numerical behavior is preserved for pure renames; the periodogram shell correction requires a failing fixture and benchmark rebaseline.
- Evidence: Audit performed against `b56cc60`; implementation sources and primary terminology references are recorded in the audit document.
- Status: Accepted; remediation may begin.

## 2026-08-22 — SCI-01/02/03/05 accurate residual and interpretation contract

- Context: The marked result, configuration, artifacts, reports, benchmarks, and source layout called a three-score neighborhood/block heuristic MODWT, wavelet, scalogram, and DoG. The same generic engine emitted MMR/clonality prose, and its marked territory DTO used ambiguous `z_or_power` plus a fabricated zero QC overlap.
- Decision: Remove the misleading names without compatibility aliases. Use `[multiscale_residual]`, `MultiscaleResidualSummary`, `ScaleEnergyPoint`, and `ResidualTerritory`. Name the three scores local-difference energy, residual energy, and block-mean variance. Name the radius helper literally and state that it performs no Gaussian filtering. Give marked residual territories distinct `analysis_scale_um`, `residual_score`, and `supporting_marked_cells` fields; make QC overlap optional. Emit neutral marked-pattern classes and prose.
- Consequences: This is an intentional format-0.3/config break documented in the migration guide. Old config keys fail and old JSON field names are absent. Marked GeoJSON and figure/Parquet artifact names change coherently. Multimodal `TerritoryFeature` remains separate pending its Phase 8 replacement; its QC overlap also becomes optional rather than fabricated zero. The engine now has explicit imports in both touched stage/assembly modules.
- Evidence: Commit `ada6159`; config and result alias tests fail on the old surface and pass on the new one. Relative-energy, residual-territory, typed-GeoJSON, report, pre/post, engine, CLI, workflow, no-default-feature, and warnings-denied Clippy checks pass. All-feature Nextest passes 292/292 with 15 expected skips. A three-sample debug-profile smoke benchmark completed for marked/multimodal territories and profiles at 256/512/1024; it is not used to claim a performance change against the release baseline.
- Status: Accepted; SCI-01, SCI-02, SCI-03, and SCI-05 closed.

## 2026-08-22 — SCI-04 Hann-tapered raster periodogram

- Context: The diagnostic called a single Hann-tapered raster FFT a Bartlett periodogram, although a Bartlett estimator averages periodograms from multiple segments. Its `low_k_shells` option selected the first individually sorted Fourier modes rather than radial shells.
- Decision: Name the implemented method `hann_tapered_raster_periodogram`. Form radial annuli with width `1 / (max(width, height) * cell_size_um)`, average every non-DC mode within each nonempty shell, and give shell means equal weight when calculating the requested low-frequency and whole-spectrum summaries. This makes `low_k_shells` behavior literal and prevents shells with more lattice modes from dominating solely by cardinality.
- Consequences: Source and documentation no longer claim a Bartlett estimator. The corrected shell aggregation intentionally changes the diagnostic value: the red known-raster regression produced `0.0905901845` under first-mode selection and `0.3376166373` under the independent first-shell oracle. The method remains a raster diagnostic, not the primary point-pattern spectrum.
- Evidence: Commit `35857b4`; the independent shell fixture, finite diagnostic, 20-test engine-spectrum suite, warnings-denied Clippy, no-default-features check, and all-feature Nextest (293/293, 15 expected skips) pass. Criterion `marked_analysis_periodogram_grid64` completed with a 12.703 ms median estimate (12.625–12.790 ms interval); formal baseline/final comparison remains Phase 12.
- Status: Accepted; SCI-04 closed.

## 2026-08-22 — SCI-06 mark-pair covariance contract

- Context: The marked distance curve was called pair correlation even though it averages centered binary-mark products and performs no point-density normalization. Its public point DTO was also reused by multimodal cross-interaction curves containing raw pair counts.
- Decision: Name the statistic, module, functions, result fields, config margin, timing stage, artifact, benchmark, and pre/post comparison `mark_pair_covariance`. Expose `MarkPairCovariancePoint { covariance, pair_count, ... }` and a separate `CrossInteractionPoint`; do not retain aliases for the misleading result or config names. Document the exact centered-product formula and explicitly distinguish it from point-process `g(r)`. Move the three touched marked permutation paths from ad hoc salts to typed seed namespaces.
- Consequences: Format 0.3 and configuration consumers must migrate the renamed keys and Parquet filename/columns. Numerical covariance behavior is unchanged, but endpoint permutation sequences change intentionally to the centralized domain-separated seed contract. PERF-04 remains open: this commit does not disguise or optimize the quadratic pair scan.
- Evidence: Commit `2449e3f`; the result-schema test was red on the old field and generic point fields, then passed with `mark_pair_covariance`, `covariance`, and `pair_count`. The obsolete config key is rejected. Formatting, warnings-denied Clippy, no-default-features, doc tests, the 20-test engine suite, CLI tests, and all-feature Nextest (295/295, 15 expected skips) pass. Release benchmark medians were 37.250, 134.917, and 415.041 µs at 256, 512, and 1,024 points, preserving evidence that indexed geometry is still required.
- Status: Accepted; SCI-06 closed.

## 2026-08-22 — SCI-07 descriptive curve-margin contract

- Context: `curve_equivalence_test` only compared one maximum standardized curve distance with a supplied threshold. It had no equivalence null hypothesis or p-value, yet format 0.2 exposed a permanently empty `p_equivalence` field and reports called the operation a formal equivalence test.
- Decision: Name the operation `curve_margin_assessment`. Format 0.3 uses `margin` and `within_margin`, deletes `p_equivalence`, and states that the comparison is descriptive. Configuration uses `[comparison.margins]` and rejects the old section. A finite zero margin consistently means exact match. Rename the synthetic validation scenario/rate to within-margin language while leaving its direct outcome synthesis visibly open under COR-01. Compile and expose the comparison module without the CLI feature.
- Consequences: Result/config consumers must migrate without aliases. `within_margin = true` is not evidence from a statistical equivalence test. A missing prespecified margin produces no boolean conclusion and an explicit unavailable interpretation. The pooled-bin difference operation remains separately identified and is addressed more fully by SCI-09.
- Evidence: Commit `2191974`; the schema regression was red on `equivalence_margin`/`p_equivalence`/`equivalent` and passes on the corrected contract. A second red test proved configured zero margins were inconsistent with the low-level API; both now accept zero. Margin, profile, pre/post, report, config, validation, and CLI coverage pass. Formatting, warnings-denied Clippy, no-default-features, doc tests, and all-feature Nextest pass (296/296, 15 expected skips).
- Status: Accepted; SCI-07 closed.

## 2026-08-22 — SCI-08 fixed-prior beta posterior diagnostic

- Context: The optional diagnostic called itself beta-binomial but only calculated separate conjugate beta posteriors for pooled and fixed groups using a Beta(1,1) prior. It did not fit beta-binomial marginal counts, a shared mixing distribution, or an overdispersion parameter.
- Decision: Use `beta_posterior_groups` in configuration and results, `BetaPosteriorSummary`/`BetaPosteriorGroupSummary` for DTOs, `beta_posterior_group_summary` for the function and diagnostic identifier, and beta-posterior-group wording in timing, CLI, reports, examples, and documentation. Preserve the exact calculation and label it exploratory. Reject obsolete config and result keys rather than aliasing them.
- Consequences: Format 0.3 consumers and configuration files require the documented rename. The diagnostic still groups by multiple component IDs when available and otherwise by coordinate-median quadrants. It must not be interpreted as evidence of spatial dependence or beta-binomial overdispersion. The touched production diagnostics stage now has explicit imports instead of a parent wildcard.
- Evidence: Commit `1e8fbbd`; the result-schema test failed on the former field and passed on `beta_posterior_groups` plus identifier `beta_posterior_group_summary_v1`. Independent posterior means/intervals, component grouping, average-even coordinate medians, config alias rejection, marked CLI/report output, and multimodal rejection pass. Formatting, warnings-denied Clippy, no-default-features, doc tests, and all-feature Nextest pass (298/298, 15 expected skips).
- Status: Accepted; SCI-08 closed.

## 2026-08-22 — SCI-09 pooled-bin comparison diagnostic

- Context: The curve “difference test” pooled two already-aggregated curves and shuffled their bin values. That operation has no spatial or per-cell exchangeability justification, while public DTO and collection names presented all comparison rows as tests.
- Decision: Use `pooled_bin_difference_diagnostic`, `pooled_bin_p_value`, `CurveComparisonResult`, `curve_comparisons`, and a typed `CurveComparisonMethod` distinguishing pooled-bin permutation, descriptive margin, and unavailable rows. Retain explicit interpretation/report text that the diagnostic is approximate and non-spatial. Keep `spectral_curve_test` unchanged because it is the separately verified ERL global-envelope test.
- Consequences: Format 0.3 consumers must migrate the renamed comparison fields/collections. The numerical shuffled-bin diagnostic and deterministic seed stream are preserved; only claims and schema ownership change. More fundamental comparison-model separation remains Phase 10 work if required.
- Evidence: Commit `69005bb`; the schema test failed on generic `p_difference` and passed with method `pooled_bin_permutation` plus `pooled_bin_p_value`. Determinism, nonzero statistic, zero permutations, axis/availability, report, JSON, multimodal CLI, formatting, warnings-denied Clippy, no-default-features, doc tests, and all-feature Nextest pass (299/299, 15 expected skips).
- Status: Accepted; SCI-09 closed.

## 2026-08-22 — Phase 3 closed with honest smoke labeling

- Closure: Public analytical names now match their implementations. False MODWT/wavelet/scalogram/DoG/Bartlett/pair-correlation/beta-binomial/equivalence/difference-test claims are removed or explicitly negated in explanatory/migration text. The generic marked engine is neutral. Established rigid, affine, spectrum, anisotropy, and ERL names retain implementation/reference tests.
- Interim COR-01 decision: Until Phase 9 replaces direct outcome synthesis, expose the current workflows only as `marklab smoke`, `SyntheticSmoke*`, and `smoke.json`; state that they are not calibration or validation evidence. Remove the Task-derived below-resolution alias. This contains the claim but does not close COR-01.
- Verification: `cargo +1.96.0 fmt --all --check`, warnings-denied all-target/all-feature Clippy, `cargo +1.96.0 check --locked --no-default-features`, all-feature doc tests, and `cargo +1.96.0 nextest run --locked --all-features` all exit 0. Nextest ran 299/299 tests with 15 expected skips. Obsolete-name searches return only negative assertions, migration history, or explicit “not this algorithm” documentation.
- Status: Accepted; Phase 4 may begin.

## 2026-08-22 — Phase 4 opened with orchestration evidence first

- Entry: Re-read Phase 4 §§11.1–11.8 after the clean Phase 3 closure at `7ecca5b`. Begin by mapping the actual marked engine stages, multimodal engine outputs, CLI-only transform/graph/geometry/sidecar calculations, feature gates, cell-table responsibilities, metadata duplication, and pre/post coupling.
- Decision: Do not start with the target directory tree. First add boundary-focused regressions for one transform fit, one graph build, and library/CLI core-result parity, then introduce the smallest application-run objects needed to make those tests pass. Preserve spectrum numerics while extracting cohesive responsibilities; ARCH-03 decomposition follows evidence from the marked workflow rather than file-size alone.
- Required constraints: Domain comparison is already no-default/CLI-independent. Further touched domain algorithms must lose CLI feature gates. Refactored production modules use explicit imports. CLI may load inputs and write outputs but may not calculate scientific sidecars.
- Status: Accepted; Phase 4 active.

## 2026-08-22 — Canonical multimodal transform, graph, and null-sensitivity run

- Context: The public engine fitted registration and built a graph for its core result, but the CLI immediately repeated both operations and independently ran the configured null-model sensitivity analyses. Library users could not obtain those sensitivity results, and stratified enrichment was compiled only with the CLI feature.
- Decision: Return a concrete `MultimodalAnalysisRun` from `analyze_run`, while preserving `analyze` as the simple core-result API. The run owns the single fitted transform, single graph, core result, and one result for every configured neighborhood null. The existing source-section primary enrichment is reused instead of recomputed. Stratified enrichment and its seed namespace are unconditional domain code. No trait or generic stage framework was introduced.
- Consequences: The CLI consumes the run and no longer fits transforms, builds graphs, derives strata, or runs null permutations. Direct library and CLI core results match. Registration residuals, convex-hull extrapolation, and scientific artifact projections remain in the CLI and keep ARCH-01/04 open for the next slice. `SpatialGraph`, `Transform2D`, and the run artifacts are public because they are explicit application-run outputs; the Phase 10 API review may narrow constructors or visibility while preserving supported run consumption.
- Evidence: Commit `698226c`; exact one-fit/one-build tests, all-null presence, direct library/CLI parity, and sidecar integration pass. Formatting, warnings-denied Clippy, no-default-feature check, doc tests, and all-feature Nextest pass; Nextest ran 303/303 tests with 15 expected skips.
- Status: Accepted; BOUND-03 and DUP-06 closed, ARCH-01/04 remain active.

## 2026-08-22 — Registration diagnostics are application artifacts with typed hull availability

- Context: The CLI independently calculated landmark residuals and convex-hull extrapolation after the public engine returned. Its local hull helper returned `true` for every point when fewer than three hull vertices existed, including collinear targets, thereby misreporting unavailable extrapolation assessment as zero cells outside the landmark hull.
- Decision: Calculate residuals and extrapolation once inside the multimodal application run. Use a reusable monotonic-chain 2-D hull with scale-normalized orientation comparisons and deterministic coordinate ordering. Represent an assessable polygon, fewer than three unique target landmarks, and collinear target landmarks as distinct typed states. Per-cell outside flags and aggregate counts/fractions are optional when assessment is unavailable; an empty cell set has an assessable hull but no defined fraction.
- Consequences: Library users receive the same residual and extrapolation artifacts previously available only to CLI users. Normal sidecar field names remain, with an added availability and unique-point count; degenerate cases intentionally change from fabricated `false`/zero values to JSON nulls. The CLI only serializes these artifacts and contains no hull or residual calculations. Scientific CSV/result projection still belongs in an output adapter, so ARCH-04 remains open.
- Evidence: Commit `b97dfb3`; boundary, ordering, numerical tolerance, empty/one/two/duplicate/collinear landmark, empty-cell, residual, and degenerate CLI sidecar tests pass. Formatting, warnings-denied Clippy, no-default-feature check, doc tests, and all-feature Nextest pass; Nextest ran 310/310 tests with 15 expected skips.
- Status: Accepted; ARCH-01 closed and ARCH-04 remains active for adapter extraction.

## 2026-08-22 — Multimodal output consumes the application run

- Context: After scientific calculations moved out of CLI, `cli/multimodal/analyze.rs` still contained every JSON/CSV sidecar projection and cloned the complete multimodal result solely to satisfy the generic writer's ownership shape.
- Decision: Add one feature-gated multimodal output adapter that consumes `MultimodalAnalysisRun`, moves its result into the versioned document, and borrows all sidecar projections from that document and the remaining run artifacts. Keep projection logic literal and format-specific; do not calculate scientific values or introduce an artifact framework before Phase 5 transaction planning. Apply the shared finite-value validator to JSON and generic CSV sidecars.
- Consequences: The multimodal CLI is 106 lines and has one application call plus one output call. No complete result or fused-cell table is cloned on this output path. Artifact order and non-atomic failure behavior are intentionally unchanged and remain visible under OUT-03. Marked output still clones its result, so PERF-10 remains open for that path.
- Evidence: Commit `a29885b`; all 21 multimodal CLI tests, nine enabled output tests, formatting, warnings-denied Clippy, and no-default-feature check pass. The immediately preceding full run at `b97dfb3` passed 310/310 with 15 expected skips; this behavior-preserving move was then covered by the complete multimodal CLI suite.
- Status: Accepted; ARCH-04 and BOUND-04 closed, PERF-10 remains active for marked output.

## 2026-08-22 — One enrichment core with explicit permutation strategy

- Context: Source-section and explicitly stratified neighborhood enrichment duplicated the entire analytical loop, including graph/config validation, label extraction, observed counts, result construction, and multiple-testing adjustment. Only the grouping used to shuffle labels differed.
- Decision: Keep the two clear public entry points, but route both through one private execution core. Inject a small internal enum holding source-section groups or explicit string strata; use one permutation-count loop that dispatches only the shuffle operation and preserves the existing domain-separated seed endpoints.
- Consequences: Observed quantities, undefined ratio/z-score rules, scalar p-values, and BH adjustment have one owner. There is no trait or speculative null-model framework. Exact deterministic sequences intentionally remain unchanged for both strategies.
- Evidence: Commit `b233104`; pinned pre-refactor JSON outputs cover both strategies and two label pairs, including a stratified zero-variance null and unstratified adjusted p-values. All 16 enrichment tests, configured-null application test, sidecar integration, formatting, warnings-denied Clippy, and no-default-feature check pass.
- Status: Accepted; DUP-04 closed.

## 2026-08-22 — Marked run owns execution context and is consumed by output

- Context: `AnalysisEngine::analyze_pattern` returned only `MarkedPatternResult`. The CLI added adapter-level load timings, cloned the complete result into `ResultDocument`, and retained the original solely for manifest fields and actual thread reporting.
- Decision: Add `MarkedAnalysisRun { result, actual_thread_count }` as the public application return while preserving `analyze_pattern` as the simple convenience API. Prepare the existing CLI manifest value while borrowing the run, then pass ownership to a marked output adapter that moves the result into the versioned document. Keep intermediate pattern exports outside the run until their Phase 5 artifact plan is defined.
- Consequences: Both output paths now avoid complete result/table clones. Strict-repro and requested-versus-actual thread reporting are preserved. This is an ownership boundary, not a claim that the marked distributed god workflow is decomposed; ARCH-02 remains open for explicit planning/computation/interpretation stages.
- Evidence: Commit `82751fe`; the public marked-run regression and all 15 enabled marked CLI integration tests pass, including strict-repro manifest, observability, intermediate, Parquet, and batch flows. Formatting, warnings-denied Clippy, and no-default-feature check pass.
- Status: Accepted; PERF-10 closed and ARCH-02 remains active.

## 2026-08-22 — Marked coordinator delegates typed spectrum and spatial stages

- Context: Even after the run ownership fix, `analyze_pattern_inner` remained a roughly 350-line distributed coordinator that directly owned spectral mode planning, observed and permutation execution, confounding sensitivity, periodogram checks, covariance, territories, anisotropy, multiscale endpoints, interpretation, diagnostics, telemetry, and assembly.
- Decision: Extract two cohesive application stages with explicit inputs and typed outputs. The spectrum stage owns mode planning, observed binary/probabilistic power, stratified/unstratified permutation execution, and null-sensitivity status mapping. The pooled-spatial stage owns the periodogram diagnostic, mark-pair covariance, residual territories, anisotropy, and multiscale residual endpoints. Keep validation/component planning, generic interpretation policy, optional diagnostics, telemetry annotation, and result assembly in their existing focused modules. Do not introduce traits or mirror the target directory mechanically.
- Consequences: `analyze_pattern_inner` is 99 lines and expresses execution order rather than numerical implementation. `api.rs` drops direct dependencies on spectrum kernels, rasters, anisotropy, and multiscale algorithms. The two new modules are 210 and 166 lines, use explicit imports, and expose only crate-private stage records. `stages.rs` remains a 444-line low-level helper collection for later ARCH-03/Phase 7 decomposition.
- Evidence: Commit `e29bd2c`; the 20-test engine-spectrum integration suite passed before and after both extractions. Formatting, warnings-denied Clippy, no-default-feature check, doc tests, and full all-feature Nextest pass; Nextest ran 313/313 with 15 expected skips.
- Status: Accepted; ARCH-02 closed.

## 2026-08-22 — Domain cells, input adapters, shared metadata, and borrowed labels

- Context: `multimodal/cell_table.rs` mixed domain types with generic CSV decoding, CellViT adaptation, validation formatting, and label policy. Each fused cell cloned three run-level strings, while every label lookup allocated a new `String` inside graph, permutation, curve, territory, and profile loops.
- Decision: Split the module by responsibility. Keep `HeCell`, `IhcCell`, `FusedCell`, `CellSection`, and `AnalysisMetadata` in the domain cell module; make label access return a borrowed H&E `&str` or static IHC label; share row validation between distinct generic CSV and CellViT adapters. Store case/timepoint/protein once in analysis metadata and move it into the result. Flatten those values into fused-cell CSV and Parquet rows only at the output boundary.
- Consequences: `FusedCell` public shape intentionally loses the three non-cell fields; it is skipped in versioned result serialization, while canonical fused-cell exports preserve their prior columns and values. Enrichment and cross-curve permutations shuffle borrowed labels; graph smoothing indexes borrowed labels without per-cell string clones; profiles allocate strings only for final result rows. The WSI adapter is untouched.
- Evidence: Commit `dc9ffeb`; pointer identity proves H&E label borrowing, domain serialization excludes run metadata, CSV rows retain all shared metadata values, Parquet output remains green, and multimodal/neighborhood/diagnostic/CLI coverage passes. Formatting, warnings-denied Clippy, no-default-feature check, doc tests, and full Nextest pass; Nextest ran 315/315 with 15 expected skips.
- Status: Accepted; BOUND-02 and PERF-09 closed.

## 2026-08-22 — Marked and multimodal pre/post services share only true semantics

- Context: `prepost/deltas.rs` was a 631-line god file containing two application workflows, scalar differences, generic curve diagnostics, spectrum/covariance/cross-interaction axis checks, territory summaries and matching, and presentation policy.
- Decision: Give marked and multimodal comparisons distinct service modules and result-family inputs. Keep anatomical comparability/wording in one context policy, all float-axis rules in one axes module, pooled-bin/margin/cross-curve orchestration in one curves module, and territory statistics/matching in one module with a private view trait that has two real implementations. Keep the tiny shared finite scalar delta at the pre/post module boundary.
- Consequences: The mixed god file is deleted; the largest production replacement is 163 lines. Marked services cannot accept multimodal results and vice versa. Axis tolerance and territory matching remain single-owned, so the split introduces no semantic duplication or strategy framework. Result schema separation remains Phase 10.
- Evidence: Commit `3ed6914`; 11 unit tests pass, including a direct multimodal service contract, harmless/material axis differences, unavailable curves, scalar permutation counts, and territory deltas. Marked and multimodal CLI pre/post tests pass. Formatting, warnings-denied Clippy, no-default-feature check, doc tests, and full Nextest pass; Nextest ran 316/316 with 15 expected skips.
- Status: Accepted; ARCH-06 closed.

## 2026-08-22 — Phase 4 closed with cohesive spectrum ownership

- Context: `src/spectra/structure_factor.rs` remained a 1,207-line god file after the marked application coordinator was reduced. It combined public result/config DTOs, Fourier kernels, resolvable-mode planning, shell aggregation, sequential/parallel/stratified permutation execution, scalar readouts, envelope/result assembly, and tests.
- Decision: Preserve the established public paths through a small facade, while assigning Fourier evaluation, modes, shells, permutation execution, and summaries/result assembly to five explicit modules. Keep numerical order and the existing `B × modes` storage unchanged in Phase 4; Phase 7 will change storage/chunking only with dedicated differential tests and benchmarks.
- Consequences: The facade is 392 lines including its numerical regression tests. Production modules range from 33 to 333 lines, have explicit dependencies, and establish real mathematical or execution ownership rather than ceremonial one-function files. No public API or serialized result changes in this slice.
- Evidence: Commits `38700da`, `89c7421`, `3b790b2`, and `8681338`; nine structure-factor numerical/property tests and all 20 engine-spectrum tests pass. Phase exit commands all returned exit 0: `cargo +1.96.0 fmt --check`; warnings-denied all-target/all-feature Clippy; no-default-features check; all-feature doc tests; and all-feature Nextest with 316/316 passed and 15 expected skips.
- Closure: ARCH-01/02/03/04/06, BOUND-02/03/04, DUP-04/06, and PERF-09/10 are fixed. The CLI contains no multimodal scientific computation, the transform and graph are each built once, library/CLI core results agree, analytical enrichment is not CLI-gated, and marked/multimodal pre/post services are separate.
- Status: Accepted; Phase 4 closed and Phase 5 may begin.

## 2026-08-22 — Phase 5 opened at the shared logical cell boundary

- Entry evidence: The ignored optional-absence regression fails because `write_pattern_parquet` always fabricates `internal_control_local = "valid"`, valid tumor/IHC flags, false exclusions, and zero QC/component IDs. CSV and Parquet loaders independently repeat mask filtering, QC counters, metadata validation, dense optional-column consistency, categorical encoding, retained arrays, nearest-neighbor geometry, and final Pattern assembly.
- Decision: Define one normalized decoded cell row with typed internal-control and exclusion state, then route both physical decoders through one `PatternBuilder`. Preserve format-specific parsing/type diagnostics in adapters, but give shared scientific/data semantics exactly one owner.
- Export contract: A `Pattern` contains retained cells and aggregate QC fractions, not excluded source rows or their per-row flags. Its Parquet projection is therefore a filtered canonical export, not a full input round trip. The API and provenance must say so; optional columns must remain nullable/absent instead of receiving fabricated meaningful values.
- Verification-first sequence: Keep the reproduced optional-absence failure red, add full logical CSV/Parquet parity around the shared builder, implement the smallest passing boundary, then rerun all loader, CLI, benchmark-compilation, and no-default-feature checks.
- Status: Accepted; Phase 5 active with DUP-05/OUT-04/OUT-05 in progress.

## 2026-08-22 — One Pattern ingestion state machine and honest filtered export

- Context: CSV and Parquet adapters duplicated more than 350 lines of scientific/data semantics, while the Pattern Parquet writer invented meaningful values that were not stored in `Pattern`.
- Decision: Make `DecodedCellRow` the typed logical schema, including typed internal-control availability/validity and grouped artifact/nonviable flags. Keep only physical decoding in CSV/Arrow adapters. Make one `PatternBuilder` own coordinate/mark validation, mask filtering, QC/exclusion policy and denominators, required metadata consistency, optional dense-column consistency, categorical encoding, retained arrays, nearest-neighbor geometry, and final Pattern invariants.
- Export semantics: Rename the writer to `write_filtered_pattern_export_parquet`. Every emitted row is a retained row, which justifies true tumor/IHC flags. Omit unavailable internal-control/exclusion fields and absent QC/component columns instead of inventing values. Embed `marklab.export_kind = filtered_canonical_pattern` and a plain-language limitation in schema metadata. A full input round trip is deliberately not offered because excluded source rows and their per-row state are absent from `Pattern`.
- Compatibility: CSV and Parquet now reject non-finite coordinates/non-binary marks at the shared boundary. Optional scientific metrics are validated only for in-mask retained rows in both formats, eliminating a prior adapter discrepancy. Valid accepted inputs and categorical first-seen encoding remain unchanged.
- Evidence: Commit `f4243cd`; the formerly ignored absence test is enabled, equivalent CSV/Parquet rows produce equal complete Patterns, all seven Parquet I/O and nine CSV loader tests pass, both simulation output tests pass, warnings-denied Clippy and no-default-features pass, and all-feature Nextest passes 317/317 with 14 expected skips.
- Status: Accepted; DUP-05, OUT-04, and OUT-05 closed.

## 2026-08-22 — Batch IDs are one safe output component

- Context: Both batch workflows joined manifest-provided IDs directly to the output root. A marked manifest ID of `../escaped` completed successfully outside the configured directory.
- Decision: Treat a batch ID as one trimmed normal path component, not a relative path. Reject blank, absolute, current/parent, forward-slash, backslash, and multi-component values. Reject an existing target symlink and, for existing targets, canonicalize and verify containment under the canonical root.
- Execution behavior: Both marked and multimodal batch flows call the same resolver. Marked batch resolves every job before starting sequential or parallel analysis, so a later invalid ID cannot follow earlier writes. Valid ID trimming and named output directories remain unchanged.
- Evidence: Commit `a8d38c5`; the formerly ignored traversal regression is enabled and green. A unit table covers blank/absolute/dot/parent/both separators and a valid trimmed ID; a Unix test covers an existing outward symlink. Valid marked sequential/parallel batch tests, the multimodal batch integration, warnings-denied Clippy, and no-default-features check pass.
- Status: Accepted; OUT-06 closed.

## 2026-08-22 — Analysis telemetry and run manifests have one owner

- Context: Marked `timings.json` appended a writer-only stage absent from `result.json`; the CLI then reread that sidecar to create external timings and trace JSONL. Three independent run-manifest constructors produced incompatible marked direct, marked CLI, and multimodal shapes.
- Decision: Analysis telemetry contains only analysis/load stages and is serialized unchanged everywhere it appears. Output-writing time is not injected into the scientific result or timing sidecar; it remains a separate output benchmark/artifact concern. External timings and trace projections serialize the in-memory stage vector directly.
- Manifest model: `RunManifest::from_document` is the only builder. It owns program/version, analysis kind, common result identity/status, output policy, and timing count. Optional typed context adds CLI command, inputs, and execution details without a second JSON constructor. Marked and multimodal manifests now share the same result/output structure.
- Consequences: The CLI no longer disables writer manifests, writes a second manifest, or reads/parses `timings.json`. When external observability output is requested, only the small timing vector is cloned before the owning analysis run is consumed; no complete result is cloned.
- Evidence: Commits `756ecbc`, `3d8ad46`, and `5d5f8d2`; enabled telemetry equality, marked/multimodal manifest assertions, external timing equality/trace, all eight analyze CLI tests, warnings-denied Clippy, no-default-features, and full Nextest 322/322 with 12 expected skips pass.
- Status: Accepted; OUT-01 and DUP-07 closed.

## 2026-08-22 — Pre/post outputs are typed format 0.3 documents

- Context: Marked and multimodal CLIs serialized a bare, unversioned `PrePostResult`. Marked accepted only explicit files; multimodal separately implemented file-or-directory resolution.
- Decision: Add `marked_prepost` and `multimodal_prepost` variants to the existing format 0.3 `AnalysisResult` envelope. The variants currently share the established descriptive payload but preserve comparison-family identity. Constructors, typed extraction, finite validation, generic writer handling, and manifest analysis kinds are explicit.
- Input/output behavior: One output-document resolver accepts either a result file or a directory containing `result.json`; both CLI services use it. `prepost.json` now contains the normal top-level format/provenance/analysis envelope, so former consumers must read `analysis.result`.
- Evidence: Commit `12d7c4c`; red/green `prepost_result_roundtrip`, file/directory resolver equality, marked and multimodal CLI version/kind/payload assertions, multimodal batch pre/post, 11 output tests, warnings-denied Clippy, and no-default-features pass.
- Status: Accepted; OUT-02 closed.

## 2026-08-22 — Run directories commit from a validated same-filesystem transaction

- Context: Generic writers created the final directory before writing. Marked intermediates and multimodal run sidecars were written after the core writer returned, so a late artifact failure left a directory that looked successful.
- Decision: Build an `ArtifactPlan` before filesystem mutation. It finite-validates and serializes the result, constructs the one run manifest, and lists required core files. Reserve a unique hidden sibling directory with `create_dir`, write every configured core and run-specific artifact there, validate required/declared-written paths, then rename the completed sibling to the final name.
- Target policy: A missing or existing empty final directory is accepted. A non-empty target, non-directory, or symbolic link is rejected and preserved; implicit destructive overwrite is not supported. Existing output paths are therefore safe by default until an explicit overwrite policy is designed.
- Failure/cleanup: The transaction owns its exact staging path and removes it on every error/drop before commit. Marked intermediates, multimodal residual/null/CSV sidecars, and marked/multimodal pre/post outputs all execute inside the transaction. Returned `OutputManifest` paths are rebased from staging to the committed final directory and checked against real files.
- Evidence: Commit `e9e87b0`; deterministic injected failure leaves no final/staging directory, non-empty sentinel preservation passes, manifest paths exist and contain no temp prefix, direct empty-directory compatibility passes, all 14 output tests, 16 marked CLI tests, 21 multimodal CLI tests, formatting, warnings-denied Clippy, no-default-features, and full Nextest 327/327 with 12 expected skips pass.
- Status: Accepted; OUT-03 and Phase 5 §§12.7–12.8 closed.

## 2026-08-22 — COR-03 null sensitivity is a persisted inference contract

- Context: The corrected spectrum stage executed distinct unstratified and stratified nulls but discarded the unstratified inference after deriving status flags. Consumers could not audit the confounding conclusion from the persisted result.
- Decision: Add a compact format-0.3 `SpectrumNullSensitivitySummary` rather than duplicate complete spectrum curves. It records typed primary-null identity, `family_wise_alpha`, typed availability for each null's `p_global` and low-k p-value, and a typed confounding conclusion. A non-stratified run is NotApplicable; mark-homogeneous strata preserve the unstratified inference and make only the stratified member InsufficientData.
- Evidence: Commit `2a7fa2b`; the pre-existing distinct-execution regression now asserts both exact persisted p-values and a full result-document round trip. Degenerate-strata and report-projection tests pass. Formatting, warnings-denied Clippy, no-default-features, output tests, and full Nextest 328/328 passed.
- Status: Accepted; COR-03 closed.

## 2026-08-22 — Format 0.3 marked and multimodal schemas are disjoint

- Context: `MarkedPatternResult` still serialized multimodal-only NotApplicable placeholders. The multimodal territory DTO retained duplicate/derived `z_or_power` and `scale_um`, misleading optional component identity, unimplemented QC overlap, and territory profiles always emitted two empty future-analysis vectors.
- Decision: Remove every multimodal field from the marked payload and stop marked pre/post from invoking cross-interaction comparison. Replace `TerritoryFeature` with `NeighborhoodTerritory { center, radius, supporting_abnormal_cells, cluster_id }`; retain `ResidualTerritory` as the distinct marked type. Remove both unimplemented QC-overlap fields and the never-produced profile enrichment/cross-curve fields without compatibility aliases.
- Consequences: This is an intentional format-0.3 contract correction documented in the migration guide. Multimodal cross-curve axis/comparison coverage now invokes the multimodal pre/post service. Full result-module decomposition remains ARCH-08/Phase 10, and real multimodal telemetry remains MODEL-02/Phase 8.
- Evidence: Commit `51564c8`; serialized marked-field absence, multimodal CLI shape, DBSCAN, profiles, GeoJSON, marked/multimodal pre/post, and output tests pass. Formatting, warnings-denied Clippy, no-default-features, doc tests, and full Nextest 329/329 with 12 expected skips pass.
- Status: Accepted; MODEL-01 fixed and the Phase 5 §12.5 schema boundary closed.

## 2026-08-22 — Phase 5 closed and Phase 6 opened

- Closure: CSV and Parquet decode into one logical row and one `PatternBuilder`; filtered Parquet export semantics are explicit; format 0.3 and both pre/post families are versioned; dual spectrum-null sensitivity is persisted; marked/multimodal schemas are disjoint; output commits transactionally; telemetry and manifests have one in-memory construction path; and batch IDs cannot escape their root.
- Verification: `cargo +1.96.0 fmt --all --check`, warnings-denied all-target/all-feature Clippy, `cargo +1.96.0 check --locked --no-default-features`, `cargo +1.96.0 test --locked --doc --all-features`, and `cargo +1.96.0 nextest run --locked --all-features` all exit 0. Nextest ran 329/329 tests with 12 expected skips; WSI integration cases were included.
- Remaining scope: The optional 0.2 converter was not implemented; the required migration document is the supported path and readers reject 0.2. Multimodal telemetry remains explicitly open for Phase 8. These do not invalidate the Phase 5 exit criteria.
- Phase 6 entry: Begin with PERF-01 backend evidence and brute-force differential contracts. Do not add a dependency or replace consumers until deterministic radius/kNN/duplicate behavior and dependency policy are documented.
- Status: Accepted; Phase 5 closed and Phase 6 active.

## 2026-08-22 — R*-tree backend for deterministic two-dimensional queries

- Context: The existing `spatial_index.rs`, nearest-neighbor summary, radius graph, kNN graph, and territory profile membership were all brute-force. A shared backend must support exact kNN and radius queries, duplicate coordinates, immutable stable indices, deterministic outputs, finite validation, and the project's Rust/license/dependency policy.
- Alternatives: A custom balanced k-d tree would avoid a dependency but add substantial pruning, tie, extreme-coordinate, and maintenance surface. `kiddo` 6.0.2 targets high-performance k-d workloads but is a materially larger, more feature/dependency-heavy implementation whose performance-oriented unsafe/SIMD surface exceeds the present need. `rstar` 0.13.0 was released in May 2026, supports bulk-loaded exact nearest/radius queries, has no default features, targets Rust 1.85, is MIT OR Apache-2.0, and its crate forbids unsafe code. Official API documentation states radius iteration order is unspecified, so backend order is never exposed.
- Decision: Use `rstar` 0.13.0 behind one `SpatialIndex2D`. Store immutable coordinates and original indices; compute reported distances with `hypot`; sort every result by distance then original index. For kNN, continue through every backend item tied at the kth squared-distance cutoff, then sort/truncate so a backend's internal tie order cannot change membership. Index radius is inclusive and a query-by-index excludes only that index, not duplicate coordinates.
- Dependency cost: Four lockfile packages are new (`rstar`, `heapless`, `hash32`, `stable_deref_trait`); existing `num-traits`, `libm`, `smallvec`, and `byteorder` are reused. `cargo deny check advisories licenses bans sources`, `cargo audit`, and `cargo machete` pass under the repository policy. Audit retains only the same three allowed pre-existing warnings. The comparable release test process RSS rises by about 0.6–0.8 MiB; package/build and final binary costs will be repeated in Phase 12.
- Performance evidence: At 256/512/1,024 points, nearest medians are 0.096/0.283/0.427 ms versus 0.167/0.578/1.904; radius graph is 0.109/0.311/0.565 versus 0.080/0.283/0.992; kNN graph is 0.172/0.635/0.841 versus 0.957/3.850/16.462. Small radius workloads pay a documented crossover cost. A retained 1,024–16,384 scaling workload and all raw methodology/results are in `PERFORMANCE_BASELINE.md`.
- Correctness evidence: Commit `3c4a255`; brute-force nearest, kNN, radius, graph, and profile oracles cover random/grid/duplicate/collinear/extreme finite coordinates, exact ties, invalid queries, combined graph union, and registration-resolution semantics. Formatting, warnings-denied Clippy, no-default-features, dependency policy, and full Nextest 337/337 with 13 expected skips pass.
- Remaining work: `MultimodalEngine` still builds separate indices for graph and profiles, and pair/territory algorithms remain unindexed. PERF-01 and PERF-06 therefore stay in progress even though PERF-02/PERF-03 are fixed.
- Status: Accepted; backend choice recorded and immediate production callers exist.

## 2026-08-22 — Explicit index reuse and mark-pair geometry plan

- Context: The first indexed endpoints still built separate fused-cell trees for graph and profiles. Mark-pair covariance recalculated all distances for observed labels and every permutation, while using the deterministic materialized radius API inside a plan caused avoidable allocation/sorting overhead.
- Decision: `MultimodalEngine` constructs one `SpatialIndex2D` after fusion and passes it explicitly to graph, DBSCAN-territory, and profile entry points. Add allocation-free radius visitors for domain hot paths while retaining and differentially testing deterministic materialized queries. Use `MarkPairCovariancePlan` as the accurate name for the fixed source/target/bin plan; store pairs in original source/target order so floating accumulation remains identical to the brute-force implementation.
- Permutation behavior: The marked stage constructs the plan once, evaluates the observed marks, and evaluates every domain-separated permutation over the same pair/bin assignments. Empty bins remain typed unavailable at the output boundary. A call-count regression proves no geometry rebuild occurs inside the permutation loop.
- Evidence: Commit `10e4932`; one-build multimodal, graph, DBSCAN-neighbor, profile-membership, pair-plan, and plan-reuse regressions pass. Formatting, warnings-denied Clippy, no-default-features, and full Nextest 341/341 with 14 expected skips pass.
- Performance: Allocation-free visits reduce hot-path overhead, but pair-plan construction still loses to the old observed-only scan at small sizes. Retained benchmarks separate build, one evaluation, and 19 evaluations; the plan crosses over between 512 and 1,024 points for 19 permutations and improves increasingly with larger permutation counts. Raw values and the observed-only regression are recorded in `PERFORMANCE_BASELINE.md`.
- Remaining work: Marked residual territories still rebuild neighborhoods per scale and permutation. PERF-05 and complete marked one-index reuse remain active; profile scaling needs a larger workload before PERF-06 closure.
- Status: Accepted; PERF-04 fixed, multimodal PERF-05 fixed, PERF-01/PERF-05/PERF-06 remain active as stated.
