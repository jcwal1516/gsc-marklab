# Current Refactor Status

Plan version: 1.0
Current repository SHA: `3c4a2551a6ba21110df8b51d98f865da5687ccdb`
Current branch: `refactor/audit-remediation`
Current phase: Phase 6 — Spatial indexing and geometry optimization
Current workstream: PERF-01/04/05/06 §§13.4–13.7 — reuse one application index and move pair/territory geometry onto explicit indexed plans
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §13.2; PERF-02/03; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: PERF-01 application-wide reuse, PERF-04/05/06, and Phase 6 §§13.4–13.8. COR-01 remains open after honest smoke labeling; MODEL-02 remains open only for multimodal telemetry.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 337/337 tests with 13 expected skips.
Dirty files: Refactor decisions, findings, status, and performance checkpoint document the verified indexed-geometry commit.
Recent decisions: `rstar` 0.13.0 is the shared backend after correctness, maintenance, license, dependency-policy, safety, memory, and three-size benchmark review. Marklab sorts all backend results by actual Euclidean distance then original index and retains all kNN cutoff ties. Nearest-neighbor, radius graph, kNN graph, and territory-profile lookup now use the index.
Unresolved technical questions: Decide the smallest explicit application geometry context that builds the fused-cell index once without turning plans into a monolithic cache. Pair/bin and territory-neighborhood plans must preserve present numeric order and errors.
Next three concrete actions: (1) add indexed-graph/profile entry points so `MultimodalEngine` builds one index for both; (2) add a red `PairCorrelationPlan` differential/permutation-reuse contract; (3) replace multimodal and marked territory neighborhood scans with indexed reusable plans
Next verification command: `cargo +1.96.0 test --locked --all-features --lib neighborhood::tests::graph_matches_bruteforce -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T04:01:39-04:00
