# Current Refactor Status

Plan version: 1.0
Current repository SHA: `968d014fac425049b07f770eddcd18ef649ae0b8`
Current branch: `refactor/audit-remediation`
Current phase: Phase 6 — Spatial indexing and geometry optimization
Current workstream: PERF-07 and Phase 6 §§13.7–13.8 — make the million-cell ingestion workload honest and bound output-sensitive geometry-plan memory
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.6; PERF-01–06; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: PERF-07 and Phase 6 §§13.7–13.8. COR-01 remains open after honest smoke labeling; MODEL-02 remains open only for multimodal telemetry.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 343/343 tests with 15 expected skips; the post-run eligible-scale change also passed both residual regressions, both affected engine tests, Clippy, and no-default-features.
Dirty files: Refactor decisions, findings, status, and performance checkpoint document the verified residual-neighborhood plan and larger scaling runs.
Recent decisions: A contiguous per-scale offsets/neighbors plan preserves cell-index summation order and is retained once across observed and permutation territory evaluations. The marked spatial stage owns one index for both pair and residual plans; only configuration-eligible territory scales are stored. Raster nulls now consume alternate marks directly instead of cloning `Pattern`.
Unresolved technical questions: Pair and broad-scale residual plans are output-sensitive and can approach quadratic storage when configured radii grow with the window. Phase 6 must add an honest configured-budget guard or a bounded-memory execution strategy before closure.
Next three concrete actions: (1) inspect and rewrite the million-cell fixture generator to stream rows and separate decoding from indexed nearest-neighbor work; (2) add actual geometry-plan storage accounting and a configured-budget regression; (3) run Phase 6 exit verification and record its closure decision
Next verification command: `rg -n 'million|1_000_000|1m|String::with_capacity|push_str' benches src tests`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T04:44:20-04:00
