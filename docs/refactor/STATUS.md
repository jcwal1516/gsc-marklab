# Current Refactor Status

Plan version: 1.0
Current repository SHA: `2449e3f94a417306c914ac55b6b6b22c47a5e9b9`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: SCI-07 curve margin-assessment naming and placeholder removal
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-06
Requirements currently in progress: SCI-07; audit findings SCI-08 and SCI-09 remain queued in Phase 3; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The full SCI-06 all-feature run passed 295/295 tests with 15 expected skips.
Dirty files: Phase 3 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` until the documentation commit
Recent decisions: The centered binary-mark product is now explicitly mark-pair covariance, with distinct mark-covariance and cross-interaction point DTOs. New result/config/artifact names have no compatibility aliases. Touched permutation endpoints use typed seed namespaces.
Unresolved technical questions: SCI-07 must define the non-inferential margin-assessment result shape and remove `p_equivalence` without implying a statistical equivalence test; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) add a failing result-schema test for margin-assessment names and absence of `p_equivalence`; (2) rename the equivalence module/functions/config/report surfaces to descriptive margin-assessment terms; (3) run pre/post, multimodal profile, serialization, and full verification suites
Next verification command: `cargo +1.96.0 test --locked --all-features --lib prepost::tests::curve_margin_assessment_has_no_equivalence_p_value -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T01:25:00-04:00
