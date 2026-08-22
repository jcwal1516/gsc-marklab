# Current Refactor Status

Plan version: 1.0
Current repository SHA: `aecc55412b59a03d2a891e0238e8df94634904c8`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: COR-04 typed undefined sparse-enrichment statistics and finite serialization
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, COR-02, and COR-03 execution/conclusion semantics
Requirements currently in progress: COR-04; COR-03's versioned sensitivity-result field is staged for Phase 5; then COR-05–COR-07 and MODEL-04
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Ten ignored `remediation_*` tests represent open findings: COR-01 engine calls, COR-04 finite enrichment and JSON round-trip, COR-05 empty bins, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. COR-02 and COR-03 reproductions are enabled and passing.
Dirty files: Phase 2 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: Stratified spectrum is the declared primary null. One unstratified sensitivity reuses the same modes and observed powers. Confounding means unstratified low-k p-value below alpha and evaluable stratified p-value at or above alpha. Homogeneous strata are a degenerate null with no numeric primary spectrum result.
Unresolved technical questions: The version 0.3 representation for both spectrum sensitivity results remains staged for Phase 5; COR-04 must choose optional statistics plus a reason-bearing undefined state without prematurely widening unrelated schemas
Next three concrete actions: (1) change enrichment ratio and z-score fields to typed optional values with explicit undefined reasons; (2) update JSON/CSV/Parquet/report consumers and enable both sparse regressions; (3) prove every sparse/zero-variance output persists without non-finite values
Next verification command: `cargo +1.96.0 test --locked --all-features --lib neighborhood::tests::remediation_sparse_enrichment_statistics_are_finite_or_typed_undefined -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T23:09:19-04:00
