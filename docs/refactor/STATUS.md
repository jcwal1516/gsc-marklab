# Current Refactor Status

Plan version: 1.0
Current repository SHA: `e7447c06646858bedd4b13fab55a5a202228eb30`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: COR-06 tolerant pre/post axis identity
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, and COR-05
Requirements currently in progress: COR-06; COR-03's versioned sensitivity-result field is staged for Phase 5; then COR-07 and MODEL-04
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Seven ignored `remediation_*` tests represent open findings: COR-01 engine calls, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. COR-02 through COR-05 reproductions are enabled and passing.
Dirty files: Phase 2 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: Pair-correlation preserves empty physical bins as `count = 0, value = null`, excludes them from inference, and emits no envelope bounds. Curve tests use typed availability, an optional statistic, and an explicit unavailable reason rather than sentinel zero.
Unresolved technical questions: The version 0.3 representation for both spectrum sensitivity results remains staged for Phase 5; COR-06 must establish and document an absolute/relative tolerance for independently reconstructed axes because the current result model does not carry a canonical axis identifier
Next three concrete actions: (1) run the ignored COR-06 reproduction and inspect every axis comparator; (2) add boundary tests distinguishing harmless reconstruction from a material mismatch; (3) implement one documented axis comparison rule and verify all pre/post paths
Next verification command: `cargo +1.96.0 test --locked --all-features --lib prepost::tests::remediation_prepost_axes_accept_harmless_float_reconstruction -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T23:33:53-04:00
