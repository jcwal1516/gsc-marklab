# Current Refactor Status

Plan version: 1.0
Current repository SHA: `8681338af1846c969c86c0eb88127f925d3ac98d`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 complete — Phase 5 entry is next
Current workstream: Phase 4 closure after application, CLI, spectrum, cell/input, metadata, output-ownership, and pre/post boundary remediation
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; ARCH-01/02/03/04/06, BOUND-02/03/04, DUP-04/06, and PERF-09/10
Requirements currently in progress: None between phases. COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Phase 4's all-feature Nextest run passed 316/316 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified Phase 4 closure.
Recent decisions: Preserve the `structure_factor` public facade while assigning Fourier kernels, mode planning, shell aggregation, permutation execution, and scalar/result summaries to explicit modules. Defer shell-level permutation storage and chunking behavior changes to Phase 7.
Unresolved technical questions: Phase 5 must choose the smallest normalized-row/`PatternBuilder` boundary that removes CSV/Parquet state-machine duplication without silently changing optional-field or export semantics.
Next three concrete actions: (1) re-read Phase 5 and inventory CSV/Parquet decode/build/write paths; (2) enable the optional-absence regression and add logical parity coverage before production changes; (3) introduce one normalized decoded row and one pattern builder with unchanged accepted-input semantics
Next verification command: `cargo +1.96.0 test --locked --all-features remediation_parquet_roundtrip_preserves_optional_absence -- --ignored --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:35:53-04:00
