# Current Refactor Status

Plan version: 1.0
Current repository SHA: `12d7c4c63740e17ac43f3cae8a4d29fc0415a575`
Current branch: `refactor/audit-remediation`
Current phase: Phase 5 — Input, schema, and output architecture
Current workstream: OUT-03 / Phase 5 §§12.7–12.8 — artifact planning and same-filesystem transactional output commit
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.4, §12.9, and §12.10; DUP-05/07, OUT-01/04/05/06
Requirements currently in progress: Phase 5 §§12.7–12.8 / OUT-03. COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. The current all-feature Nextest run passed 322/322 tests with 12 expected skips.
Dirty files: README, SPEC, result-format, migration, and refactor ledgers document the verified OUT-02 schema change.
Recent decisions: Format 0.3 has distinct `marked_prepost` and `multimodal_prepost` variants. Both CLI workflows accept a result file or directory through one resolver and write a finite-validated versioned `prepost.json`.
Unresolved technical questions: Define overwrite behavior and a testable failure injection for same-filesystem temporary-directory output commits without weakening existing refusal/preservation semantics.
Next three concrete actions: (1) reproduce partial-output visibility with a deterministic artifact-write failure; (2) introduce a temporary sibling transaction that owns all writes; (3) validate required artifacts and rename the complete directory into place with cleanup on failure
Next verification command: `cargo +1.96.0 test --locked --all-features failed_artifact_write_does_not_commit_final_directory -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T03:09:06-04:00
