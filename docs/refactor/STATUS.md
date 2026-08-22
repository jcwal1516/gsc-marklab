# Current Refactor Status

Plan version: 1.0
Current repository SHA: `1ff9459aa260096bd14c737c7cec093af243bd66`
Current branch: `refactor/audit-remediation`
Current phase: Phase 0 — Baseline, evidence, and reproducibility
Current workstream: Performance-baseline coverage and measurement
Last completed requirement IDs: Phase 0 §§7.1–7.4; COR-01–COR-07, MODEL-04, OUT-01, OUT-04/05, and OUT-06 have explicit failing regressions or direct reproduction evidence
Requirements currently in progress: Phase 0 §7.5 benchmark coverage, scaling measurements, peak-memory capture, and baseline report
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Twelve ignored `remediation_*` tests fail intentionally when run with `--ignored`: COR-01 engine calls, COR-02 rotation, COR-03 distinct nulls, COR-04 finite enrichment and JSON round-trip, COR-05 empty bins, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Exact evidence is in `docs/refactor/REGRESSION_REPRODUCTIONS.md`.
Dirty files: Phase 0 regression changes in `src/io/parquet_tests.rs`, `src/multimodal/{engine.rs,mod.rs}`, `src/neighborhood/tests.rs`, `src/output/tests.rs`, `src/prepost/tests.rs`, `src/registration/tests.rs`, `src/spectra/tests.rs`, `src/validation/tests.rs`, `tests/{cli.rs,engine_spectrum.rs}`; records in `docs/refactor/{BASELINE_VERIFICATION.md,FINDINGS_MATRIX.md,REGRESSION_REPRODUCTIONS.md,STATUS.md}`
Recent decisions: Preserve the dirty `branch/spatial-phenotype-recovery` checkout untouched; use the clean linked worktree. Treat RUSTSEC-2026-0253 as a real transitive risk but not a currently triggerable Marklab path because the reachable WSI cache keys have non-panicking drops and no relevant `catch_unwind`; evaluate the patched dependency separately from baseline capture.
Unresolved technical questions: Precise result-0.3 availability representation, confounding sensitivity result ownership, spatial-index backend, benchmark peak-memory mechanism, feasible pre-index baseline sizes, and safe transitive `lru` upgrade path remain undecided pending measurements
Next three concrete actions: (1) commit the verified Phase 0 failing-regression checkpoint; (2) add missing benchmark coverage with correctness checks and at least three scaling sizes; (3) run the baseline benchmarks and record wall time, scaling, memory, density, edge counts, permutations, threads, profile, and SHA
Next verification command: `cargo +1.96.0 bench --locked --all-features --no-run`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T22:13:47-04:00
