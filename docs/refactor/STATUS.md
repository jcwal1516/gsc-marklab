# Current Refactor Status

Plan version: 1.0
Current repository SHA: `35857b4c08c7bda1e0bcc8e60c6056dce5cdcfe0`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: SCI-06 mark-pair covariance naming
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-05
Requirements currently in progress: SCI-06; audit findings SCI-07 through SCI-09 remain queued in Phase 3; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The full SCI-04 all-feature run passed 293/293 tests with 15 expected skips.
Dirty files: Phase 3 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` until the documentation commit
Recent decisions: The former Bartlett diagnostic is a Hann-tapered single-raster periodogram. Radial shells use the longer raster dimension's minimum Fourier spacing, average all modes in each nonempty annulus, and give shell means equal weight.
Unresolved technical questions: SCI-06 must coordinate public DTO, config, artifact, report, and API names without retaining a misleading compatibility alias; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) add a failing format-0.3 assertion for the `mark_pair_covariance` field and absence of `pair_correlation`; (2) rename the centered-product implementation and all public/artifact/config surfaces; (3) run focused numerical/output tests and the renamed baseline benchmark
Next verification command: `cargo +1.96.0 test --locked --all-features --lib output::tests::result_uses_mark_pair_covariance_schema -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T00:50:00-04:00
