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
