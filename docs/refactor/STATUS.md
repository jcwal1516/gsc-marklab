# Current Refactor Status

Plan version: 1.0
Current repository SHA: `53e234849351b0fb520d2eb9a91dcd1fd34dbbaf`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: COR-03 distinct unstratified and stratified confounding sensitivity
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, and COR-02
Requirements currently in progress: COR-03, followed by COR-04–COR-07 and MODEL-04
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Eleven ignored `remediation_*` tests represent open findings: COR-01 engine calls, COR-03 distinct nulls, COR-04 finite enrichment and JSON round-trip, COR-05 empty bins, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. COR-02's former ignored rotation reproduction is now an enabled passing test.
Dirty files: Phase 2 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: `RegistrationTransform::Rigid` now uses a normalized closed-form orientation-preserving least-squares rotation plus translation. The unused scale-plus-translation implementation was deleted; rigid output metadata is now `rigid`, while affine remains `affine`.
Unresolved technical questions: The narrowest result type for primary-versus-sensitivity spectrum nulls, how to report homogeneous-stratum degeneracy without prematurely committing the 0.3 schema, and how much observed spectrum planning can be reused before the Phase 7 decomposition
Next three concrete actions: (1) trace the primary spectrum and `stratified_confounds` data flow and current result consumers; (2) add the complete confounding contract tests around distinct analyses and degenerate strata; (3) implement a typed internal sensitivity result that reuses modes and observed power
Next verification command: `cargo +1.96.0 test --locked --all-features --test engine_spectrum remediation_stratified_confounding_compares_distinct_nulls -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T22:58:45-04:00
