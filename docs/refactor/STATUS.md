# Current Refactor Status

Plan version: 1.0
Current repository SHA: `e7f91ca78cb05ec0e888023f9d412819b91e79de`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: COR-07 independent QC counters and denominators
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, and COR-06
Requirements currently in progress: COR-07; COR-03's versioned sensitivity-result field is staged for Phase 5; then MODEL-04
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Six ignored `remediation_*` tests represent open findings: COR-01 engine calls, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. COR-02 through COR-06 reproductions are enabled and passing.
Dirty files: Phase 2 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: Pre/post axes from independent result documents match under `|a-b| <= 1e-12 + 1e-12 * max(|a|, |b|)`; non-finite or materially different axes retain typed mismatch diagnostics.
Unresolved technical questions: The version 0.3 representation for both spectrum sensitivity results remains staged for Phase 5; COR-07 must define the exact availability semantics and denominator for every QC fraction before unifying CSV and Parquet construction in Phase 5
Next three concrete actions: (1) run the ignored COR-07 reproduction and inspect CSV/Parquet loader state machines; (2) add behavior tests for each exclusion/counter combination and zero denominators; (3) implement shared counter semantics without prematurely performing the Phase 5 builder rewrite
Next verification command: `cargo +1.96.0 test --locked --all-features --lib validation::tests::remediation_internal_control_fraction_is_not_final_retained_fraction -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T23:38:47-04:00
