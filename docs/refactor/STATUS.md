# Current Refactor Status

Plan version: 1.0
Current repository SHA: `82751fe8207aadf225ce6d62eede7c2fe7c237fe`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-02 — extract explicit marked planning and computation stage boundaries from `analyze_pattern_inner`
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-02/03/06, BOUND-02, PERF-09; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. ARCH-01/04, BOUND-03/04, DUP-04/06, and PERF-10 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 310/310 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `82751fe` marked-run/output ownership slice before their documentation commit.
Recent decisions: `MarkedAnalysisRun` is the public application lifetime for the marked result plus actual thread count. The simple result API delegates to it, and marked output consumes it after CLI load-timing/manifest preparation. No production output path clones a complete result or fused table.
Unresolved technical questions: Choose cohesive stage records that reduce the 400+ line marked coordinator without merely relocating parent scope; decide whether load timings enter a future authoritative run telemetry object in Phase 5.
Next three concrete actions: (1) record explicit inputs/outputs for spectrum planning/execution and downstream spatial diagnostics; (2) extract one cohesive stage record with differential engine coverage; (3) remove parent wildcard imports from the touched stage module
Next verification command: `cargo +1.96.0 test --locked --all-features --test engine_spectrum`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T01:54:53-04:00
