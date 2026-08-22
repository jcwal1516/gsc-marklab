# Current Refactor Status

Plan version: 1.0
Current repository SHA: `1e8fbbd11fd2babbd3220b4eff25fc82102adfc8`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: SCI-09 pooled-bin difference-diagnostic naming
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-08
Requirements currently in progress: SCI-09; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The full SCI-08 all-feature run passed 298/298 tests with 15 expected skips.
Dirty files: Phase 3 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` until the documentation commit
Recent decisions: The former beta-binomial diagnostic is now `beta_posterior_groups`: independent fixed-Beta(1,1)-prior prevalence posteriors for pooled and component/quadrant groups. No shared dispersion model or spatial evidence claim is made.
Unresolved technical questions: SCI-09 must choose result/function/field names that expose the pooled-bin shuffling approximation and remove remaining “test” terminology without implying spatial exchangeability; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) add a failing schema/API test for pooled-bin difference diagnostic terminology; (2) rename the difference module/function/result fields and comparison collections while retaining limitation text; (3) run deterministic diagnostic, pre/post, report, serialization, and full verification suites
Next verification command: `cargo +1.96.0 test --locked --all-features --lib prepost::tests::pooled_bin_difference_uses_diagnostic_schema -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:20:00-04:00
