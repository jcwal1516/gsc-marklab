# Current Refactor Status

Plan version: 1.0
Current repository SHA: `698226c2bb2df51ad1f372319ed1e5d95a72ddd6`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-01 and ARCH-04 — move registration residual/extrapolation analysis out of CLI and complete the multimodal run boundary
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-01/02/03/04/06, BOUND-02, DUP-04, PERF-09; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. BOUND-03 and DUP-06 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 303/303 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `698226c` application-run slice before their documentation commit.
Recent decisions: `MultimodalAnalysisRun` is the application-owned lifetime for a canonical transform, graph, primary result, and configured null sensitivities. CLI output consumes those values and does not refit or rebuild them. Stratified enrichment is domain behavior and is available without the CLI feature.
Unresolved technical questions: Define typed degenerate convex-hull/extrapolation behavior while moving registration residuals and extrapolation into the run; then decide the narrow output projection boundary without preempting Phase 5 transactions.
Next three concrete actions: (1) add registration-extrapolation boundary and degenerate-hull tests; (2) move residual/extrapolation computation and typed records into a cohesive multimodal application/domain module; (3) extend `MultimodalAnalysisRun` and delete the calculations from CLI
Next verification command: `cargo +1.96.0 test --locked --all-features registration_extrapolation -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T04:05:00-04:00
