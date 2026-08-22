# Current Refactor Status

Plan version: 1.0
Current repository SHA: `3ed6914d9bb8ad59fcdd5a0195900ffbd8113cd8`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-03 — decompose `structure_factor.rs` by kernels, modes/shells, permutation execution, summaries, and result construction
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-03; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. ARCH-01/02/04/06, BOUND-02/03/04, DUP-04/06, and PERF-09/10 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 316/316 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `3ed6914` pre/post service split before their documentation commit.
Recent decisions: Marked and multimodal pre/post services are distinct. Comparability policy, tolerant axes, curve diagnostics, and territory matching/statistics are shared only where semantics are identical.
Unresolved technical questions: Decompose the spectral god file without changing numerical order before the Phase 7 storage/chunking optimization; identify private function clusters and keep public entry points stable.
Next three concrete actions: (1) map `structure_factor.rs` functions/types/tests and dependency clusters; (2) add or identify differential kernel/shell/permutation coverage; (3) move one cluster at a time with explicit imports and unchanged public re-exports
Next verification command: `cargo +1.96.0 test --locked --all-features spectra::structure_factor::tests -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:21:40-04:00
