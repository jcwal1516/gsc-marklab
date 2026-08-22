# Current Refactor Status

Plan version: 1.0
Current repository SHA: `8cacfbc4fa838827ef8942fb1bb21b4266ea8c01`
Current branch: `refactor/audit-remediation`
Current phase: Phase 5 — Input, schema, and output architecture
Current workstream: DUP-05 / OUT-04 / OUT-05 — normalized decoded rows, one `PatternBuilder`, logical CSV/Parquet parity, and explicit filtered-export semantics
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; ARCH-01/02/03/04/06, BOUND-02/03/04, DUP-04/06, and PERF-09/10
Requirements currently in progress: Phase 5 §§12.1–12.4; DUP-05, OUT-04, and OUT-05. COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Phase 4's all-feature Nextest run passed 316/316 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record Phase 5 entry evidence.
Recent decisions: Decode physical CSV and Arrow representations into one row model, while retaining typed availability for optional QC columns. One builder will own mask/QC/filtering, metadata, dense optional columns, strata, geometry, and final invariants.
Unresolved technical questions: A retained `Pattern` cannot reconstruct excluded source rows or per-row QC flags; its Parquet writer therefore needs an explicitly named filtered canonical export rather than a false full-roundtrip contract.
Next three concrete actions: (1) enable the optional-absence regression; (2) introduce the normalized row and builder and route both loaders through it; (3) rename the Pattern writer as a filtered export and preserve absent optional fields without fabricated zero/valid values
Next verification command: `cargo +1.96.0 test --locked --all-features remediation_parquet_roundtrip_preserves_optional_absence -- --ignored --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:38:18-04:00
