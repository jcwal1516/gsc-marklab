# Current Refactor Status

Plan version: 1.0
Current repository SHA: `4bf20e87407bae7bbd71aa255c3a2bbebd225d6a`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: COR-05 typed unavailable comparison and pair-correlation states
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, and COR-04
Requirements currently in progress: COR-05; COR-03's versioned sensitivity-result field is staged for Phase 5; then COR-06, COR-07, and MODEL-04
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Eight ignored `remediation_*` tests represent open findings: COR-01 engine calls, COR-05 empty bins, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. COR-02, COR-03, and both COR-04 reproductions are enabled and passing.
Dirty files: Phase 2 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: Result format 0.3 is active because COR-04 changes numeric fields to nullable typed states. Sparse ratios use `zero_expected_edges`; z-scores distinguish zero variance, insufficient null samples, and defensive non-finite computation. P-values remain available independently.
Unresolved technical questions: The version 0.3 representation for both spectrum sensitivity results remains staged for Phase 5; COR-05 must choose whether unavailable curve tests become tagged sections or a separate typed comparison record while minimizing pre/post schema churn
Next three concrete actions: (1) make pair-correlation bin values optional with explicit contributing-pair availability; (2) replace comparison `statistic = 0.0` error DTOs with tagged unavailable results; (3) update result 0.3 JSON/Parquet/report projections and enable the COR-05 regression
Next verification command: `cargo +1.96.0 test --locked --all-features --lib spectra::tests::remediation_pair_correlation_does_not_report_empty_bins_as_observed_zero -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T23:21:12-04:00
