# Current Refactor Status

Plan version: 1.0
Current repository SHA: `51564c866416302fddb5d7d0f5bf6a228fac8310`
Current branch: `refactor/audit-remediation`
Current phase: Phase 6 — Spatial indexing and geometry optimization
Current workstream: PERF-01 §§13.1–13.2 — choose one deterministic spatial-index backend and replace quadratic nearest-neighbor geometry with differential coverage
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: PERF-01 and Phase 6 §§13.1–13.2. COR-01 remains open after honest smoke labeling; MODEL-02 remains open only for multimodal telemetry.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 329/329 tests with 12 expected skips.
Dirty files: Refactor tracking documents record Phase 5 closure and Phase 6 entry.
Recent decisions: Format 0.3 persists both spectrum null inferences and has disjoint marked/multimodal schemas. `NeighborhoodTerritory` and `ResidualTerritory` have distinct completed fields; unimplemented profile/QC placeholders were removed. Phase 5 output commits are same-filesystem transactions using one artifact plan, telemetry history, and manifest builder.
Unresolved technical questions: Select the spatial-index backend after evaluating deterministic exact kNN/radius behavior, license/dependency policy, memory, and measured scaling. No new dependency is authorized without that evidence.
Next three concrete actions: (1) inventory every spatial query consumer and existing coordinate/error contracts; (2) add a deterministic brute-force oracle and red differential tests for the required `SpatialIndex2D` operations; (3) evaluate an existing dependency versus an internal exact index with three-size radius/kNN measurements
Next verification command: `cargo +1.96.0 test --locked --all-features --lib geom::tests::mean_nearest_neighbor_distance_uses_each_points_closest_neighbor -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T03:40:28-04:00
