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
