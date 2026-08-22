# Current Refactor Status

Plan version: 1.0
Current repository SHA: `f4243cdad9b7771debf39f078e97bde8fb6827e6`
Current branch: `refactor/audit-remediation`
Current phase: Phase 5 — Input, schema, and output architecture
Current workstream: OUT-06 — reject unsafe batch manifest identifiers before joining output paths
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.4; DUP-05, OUT-04, OUT-05
Requirements currently in progress: Phase 5 §12.10 / OUT-06. COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Three ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, and OUT-06 traversal. The current all-feature Nextest run passed 317/317 tests with 14 expected skips.
Dirty files: `src/io/parquet.rs` adds the filtered-export contract comment; refactor status, decisions, matrix, and reproduction ledger record the verified ingestion/export slice.
Recent decisions: `DecodedCellRow` is the authoritative logical cell boundary. CSV and Arrow adapters only map physical representations; `PatternBuilder` exclusively owns filtering, QC, metadata, dense-option, strata, geometry, and finalization semantics. Pattern Parquet output is explicitly a retained-cell export with schema provenance.
Unresolved technical questions: Batch IDs appear to be intended as one directory component; confirm all marked and multimodal batch joins share one validator without changing valid ID behavior.
Next three concrete actions: (1) run and enable the OUT-06 traversal regression; (2) centralize a single-component batch ID validator for both batch workflows; (3) test blank, absolute, separator, dot, and parent components plus valid IDs
Next verification command: `cargo +1.96.0 test --locked --all-features remediation_batch_id_cannot_escape_output_root -- --ignored --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:49:38-04:00
