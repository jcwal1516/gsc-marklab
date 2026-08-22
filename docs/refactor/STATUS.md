# Current Refactor Status

Plan version: 1.0
Current repository SHA: `dc9ffeb19d0a9e0dd53a6c5e43ff0eb1f658d945`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: ARCH-06 — separate marked and multimodal pre/post workflows, axis validation, territory matching, and interpretation policy
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-03/06; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. ARCH-01/02/04, BOUND-02/03/04, DUP-04/06, and PERF-09/10 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 315/315 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `dc9ffeb` cell/metadata/label boundary before their documentation commit.
Recent decisions: Domain cells, label policy, generic CSV input, CellViT adaptation, and row validation have separate owners. Fused cells contain no run metadata; adapters flatten shared metadata. Label views borrow H&E strings or return static IHC labels.
Unresolved technical questions: Determine the smallest pre/post split that preserves shared curve-axis/statistical semantics without duplicating policy, and decide whether the current mixed result type must wait for Phase 10.
Next three concrete actions: (1) inventory marked versus multimodal functions/types in `prepost/deltas.rs`; (2) add service-level tests proving each workflow depends only on its result family; (3) extract axis validation, territory matching, and interpretation policy with explicit imports
Next verification command: `cargo +1.96.0 test --locked --all-features --lib prepost::tests -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:14:41-04:00
