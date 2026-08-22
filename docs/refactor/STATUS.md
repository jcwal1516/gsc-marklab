# Current Refactor Status

Plan version: 1.0
Current repository SHA: `5d5f8d207b85811989fc94e1664d7d63d6174869`
Current branch: `refactor/audit-remediation`
Current phase: Phase 5 — Input, schema, and output architecture
Current workstream: OUT-02 — version marked and multimodal pre/post documents and centralize file/directory input resolution
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.4, §12.9, and §12.10; DUP-05/07, OUT-01/04/05/06
Requirements currently in progress: Phase 5 §12.6 / OUT-02. COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 322/322 tests with 12 expected skips.
Dirty files: Refactor status, decisions, matrix, and reproduction ledger record the verified telemetry/manifest unification.
Recent decisions: Result and timing sidecars contain analysis telemetry only and derive from the same in-memory stages. Output cost is not a scientific stage. External timing/trace projections use the in-memory vector, and one typed builder creates marked, multimodal, direct-library, and CLI-enriched run manifests.
Unresolved technical questions: Choose one versioned comparison envelope that distinguishes marked and multimodal pre/post while retaining the existing safe descriptive `PrePostResult` payload and consistent input errors.
Next three concrete actions: (1) add failing marked/multimodal pre/post round-trip and unversioned-output tests; (2) extend the versioned result envelope with distinct comparison variants; (3) centralize result file/directory resolution for both CLI commands
Next verification command: `cargo +1.96.0 test --locked --all-features --test result_v03 prepost_result_roundtrip -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T03:03:40-04:00
