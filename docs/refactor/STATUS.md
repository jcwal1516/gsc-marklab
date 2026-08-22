# Current Refactor Status

Plan version: 1.0
Current repository SHA: `7ecca5b729c38a38cac265b468b7aac0104da1f5`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-01, ARCH-02, ARCH-04, and DUP-06 orchestration inventory
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-01/02/03/04/06, BOUND-02/03, DUP-06, PERF-09; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The Phase 3 exit all-feature run passed 299/299 tests with 15 expected skips.
Dirty files: Phase 4 entry records in `docs/refactor/STATUS.md` and `docs/refactor/DECISIONS.md` until the documentation commit
Recent decisions: Curve comparisons distinguish pooled-bin permutation diagnostics from descriptive margins through a typed method and exact field names. The interim synthetic command is `smoke`, not validation, and documents that multimodal outcomes still bypass production.
Unresolved technical questions: Phase 4 must define cohesive marked/multimodal run objects and move transform/graph/domain sidecar ownership out of CLI without a big-bang directory rewrite; COR-03 persisted sensitivity reporting remains Phase 5 work
Next three concrete actions: (1) reread Phase 4 and inventory current marked/multimodal orchestration boundaries plus duplicate transform/graph construction; (2) add tests counting one transform fit and one graph build per public multimodal application run; (3) introduce the smallest application-run object that lets CLI consume existing computed artifacts
Next verification command: `cargo +1.96.0 test --locked --all-features --test multimodal_cli multimodal_analyze_writes_registration_and_neighborhood_outputs -- --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T03:12:00-04:00
