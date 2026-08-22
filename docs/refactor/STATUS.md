# Current Refactor Status

Plan version: 1.0
Current repository SHA: `ada61594c9e4fd3f09c599b1a6a4cb88796c462e`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: SCI-04 Hann-tapered raster periodogram and radial shells
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, SCI-01, SCI-02, SCI-03, and SCI-05
Requirements currently in progress: SCI-04; audit findings SCI-06 through SCI-09 remain queued in Phase 3; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Every Phase 2 correctness reproduction is enabled and passing.
Dirty files: Phase 3 progress records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` until the documentation commit
Recent decisions: False wavelet/MODWT/scalogram/DoG names and artifacts are removed without aliases. Marked residual territories have a distinct schema with a nullable QC overlap. Generic marked interpretation and reports now use neutral spatial language.
Unresolved technical questions: SCI-04 must define deterministic radial-shell membership and whether shell power is a mean or sum before replacing the current first-sorted-mode statistic; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) add a failing known-mode test proving `low_k_shells` groups radial shells rather than individual modes; (2) rename Bartlett types/functions/modules and implement deterministic shell aggregation; (3) run the periodogram benchmark and numerical integration suite
Next verification command: `cargo +1.96.0 test --locked --all-features --lib algorithm_tests::tapered_periodogram_groups_all_modes_in_each_radial_shell -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T00:25:07-04:00
