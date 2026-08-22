# Current Refactor Status

Plan version: 1.0
Current repository SHA: `b97dfb391ba9ab451bbfedffb2a88eef644cc87c`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-04 and DUP-04 — move multimodal projections/decoding behind adapters and consolidate enrichment execution
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-02/03/04/06, BOUND-02/04, DUP-04, PERF-09; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. ARCH-01, BOUND-03, and DUP-06 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 310/310 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `b97dfb3` registration-artifact move before their documentation commit.
Recent decisions: Registration residuals and extrapolation are application-run artifacts. Convex-hull assessment is scale-normalized and order-independent; fewer than three unique targets and collinear targets are distinct typed unavailable states. Empty cell sets have an assessable hull but undefined fraction.
Unresolved technical questions: Choose the narrow multimodal output-adapter API that removes result cloning from CLI without preempting Phase 5 transactions; consolidate enrichment through one execution core without obscuring its permutation policy.
Next three concrete actions: (1) move multimodal result/sidecar projections into a focused output adapter consumed in one CLI call; (2) add a shared enrichment-core differential test and consolidate the two wrappers; (3) inventory the marked `AnalysisEngine` stage ownership for the smallest `MarkedAnalysisRun`
Next verification command: `cargo +1.96.0 test --locked --all-features --test multimodal_cli multimodal_analyze_writes_qc_csv_and_null_sensitivity_sidecars -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T01:41:29-04:00
