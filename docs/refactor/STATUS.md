# Current Refactor Status

Plan version: 1.0
Current repository SHA: `10e493269bcc2cdbc752dc0f4b4e5c74c289d2b5`
Current branch: `refactor/audit-remediation`
Current phase: Phase 6 — Spatial indexing and geometry optimization
Current workstream: PERF-05 §§13.5–13.7 — build one marked residual-territory neighborhood plan and reuse it across observed/permuted labels
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §13.2; PERF-02/03; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: PERF-01 marked-run reuse, PERF-05, PERF-06 scaling acceptance, and Phase 6 §§13.5–13.8. COR-01 remains open after honest smoke labeling; MODEL-02 remains open only for multimodal telemetry.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 341/341 tests with 14 expected skips.
Dirty files: Refactor decisions, findings, status, and performance checkpoint document indexed application reuse and the fixed pair plan.
Recent decisions: `MultimodalEngine` builds one index and passes it explicitly to graph, DBSCAN, and profile stages. Hot paths use allocation-free index visitors; deterministic materialized APIs remain tested. `MarkPairCovariancePlan` preserves source/target summation order and is built once for observed plus all permutation curves.
Unresolved technical questions: Choose a compact residual-territory neighborhood-plan layout that reuses the marked index without `Vec<Vec<_>>`, preserves center-index summation order, and remains within the configured memory budget at the broadest scale.
Next three concrete actions: (1) add a brute-force differential contract for a contiguous residual-territory neighborhood plan; (2) pass that plan through observed and scalar-permutation territory calculations without cloning Pattern for territory work; (3) add larger graph/profile/territory scaling workloads and record output-sensitive ratios
Next verification command: `cargo +1.96.0 test --locked --all-features --lib api::stages::tests::pair_geometry_is_reused_for_observed_and_permutations -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T04:22:55-04:00
