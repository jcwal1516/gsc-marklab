# Current Refactor Status

Plan version: 1.0
Current repository SHA: `6000cc885bdbe62933560cb26eb03ed430331473`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: MODEL-04 behaviorally distinct component modes
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, and COR-07
Requirements currently in progress: MODEL-04; COR-03's versioned sensitivity-result field is staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Five ignored `remediation_*` tests represent open findings: COR-01 engine calls, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. COR-02 through COR-07 reproductions are enabled and passing.
Dirty files: Phase 2 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: Both input adapters use one QC counter model. Every fraction uses in-mask cells as denominator; validity and exclusion states are independent; `valid_mask_fraction` is final retained fraction; blank control states are invalid; zero denominators are errors.
Unresolved technical questions: The version 0.3 representation for both spectrum sensitivity results remains staged for Phase 5; MODEL-04 must define the result availability and primary-endpoint meaning for `Separate` and the selection record for `Auto`
Next three concrete actions: (1) run the ignored MODEL-04 reproduction and trace component-mode dispatch/assembly; (2) add explicit Pooled, Separate, Both, and Auto behavior tests including selection reason; (3) implement the smallest schema-correct mode distinction and run the engine spectrum suite
Next verification command: `cargo +1.96.0 test --locked --all-features --test engine_spectrum remediation_separate_component_mode_does_not_behave_like_both -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T23:47:58-04:00
