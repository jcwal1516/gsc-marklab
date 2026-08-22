# Current Refactor Status

Plan version: 1.0
Current repository SHA: `2191974dad9d0d759bd6dea7c334cbe64be23a29`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: SCI-08 beta posterior group-summary naming
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-07
Requirements currently in progress: SCI-08; audit finding SCI-09 remains queued in Phase 3; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The full SCI-07 all-feature run passed 296/296 tests with 15 expected skips.
Dirty files: Phase 3 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` until the documentation commit
Recent decisions: Curve distance versus a margin is now a descriptive margin assessment, not an equivalence test. Format 0.3 removes `p_equivalence`, uses `margin`/`within_margin`, and config uses `[comparison.margins]`. Exact-match zero margins are valid.
Unresolved technical questions: SCI-08 must choose exact public/result/config/report names for independent fixed-prior beta posterior summaries without suggesting beta-binomial dispersion modeling; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) add a failing result/config schema test for beta posterior group terminology and absence of beta-binomial names; (2) rename the diagnostic module, types, fields, CLI output, and documentation while preserving the posterior calculation; (3) run focused diagnostic, CLI, serialization, and full verification suites
Next verification command: `cargo +1.96.0 test --locked --all-features --test diagnostics_interfaces beta_posterior_group_summary_uses_accurate_schema -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T01:55:00-04:00
