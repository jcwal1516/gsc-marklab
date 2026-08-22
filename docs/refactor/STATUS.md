# Current Refactor Status

Plan version: 1.0
Current repository SHA: `e9e87b0e238278a8ad74087737144c1cdb43413c`
Current branch: `refactor/audit-remediation`
Current phase: Phase 5 — Input, schema, and output architecture
Current workstream: COR-03 / Phase 5 §12.5 — persist distinct unstratified and stratified spectrum-null sensitivity in format 0.3
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.4, §12.9, and §12.10; DUP-05/07, OUT-01/04/05/06
Requirements currently in progress: COR-03 persisted sensitivity reporting and Phase 5 result-schema completion. COR-01 remains open after honest smoke labeling.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 322/322 tests with 12 expected skips.
Dirty files: README, SPEC, refactor status, decisions, and findings matrix document the verified output transaction.
Recent decisions: `ArtifactPlan` validates result JSON, the canonical run manifest, and required core paths before commit. All configured marked, multimodal, intermediate, and pre/post artifacts write to a same-parent temporary directory; required and manifest paths are validated, then rebased and committed by rename. Non-empty/symlink targets are preserved and rejected.
Unresolved technical questions: The internal spectrum stage already produces both null results; choose the narrow result DTO that exposes availability, primary-null identity, and confounding conclusion without serializing private mode-level data.
Next three concrete actions: (1) add a red format-0.3 round-trip test for dual spectrum null sensitivity; (2) map internal `SpectrumNullSensitivity` into a public summary field; (3) update reports/migration and close COR-03 if both analyses and degenerate states persist
Next verification command: `cargo +1.96.0 test --locked --all-features --test engine_spectrum distinct_nulls_are_actually_executed -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T03:19:38-04:00
