# Current Refactor Status

Plan version: 1.0
Current repository SHA: `b233104d73842246608febbcb8dac20241247b67`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-02 — inventory marked stage ownership and introduce the smallest marked application run
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-02/03/06, BOUND-02, PERF-09/10; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. ARCH-01/04, BOUND-03/04, DUP-04, and DUP-06 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 310/310 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `b233104` enrichment-core consolidation before their documentation commit.
Recent decisions: Stratified and unstratified enrichment share one execution core. Only the permutation grouping and seed namespace differ; deterministic pre-refactor outputs are pinned for both paths.
Unresolved technical questions: Define a marked run/output lifetime that preserves observability, intermediate exports, and authoritative telemetry while eliminating the marked result clone; choose stage splits by actual input/output ownership rather than the current file boundaries.
Next three concrete actions: (1) inventory marked `AnalysisEngine` stage inputs/outputs and observability dependencies; (2) add a marked run/output ownership regression; (3) introduce the smallest `MarkedAnalysisRun` plus consuming output path
Next verification command: `cargo +1.96.0 test --locked --all-features --test cli analyze_cli_writes_result_json_from_csv_and_geojson_mask -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T01:51:16-04:00
