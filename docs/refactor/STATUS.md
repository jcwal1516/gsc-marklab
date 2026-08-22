# Current Refactor Status

Plan version: 1.0
Current repository SHA: `adda397534eac415419c7111a0b23d699b4c1bf0`
Current branch: `refactor/audit-remediation`
Current phase: Phase 11 — God-file and code-shape cleanup
Current workstream: ARCH-09 §§18.1–18.5 — inventory remaining large files/functions, distributed workflow coupling, ceremonial modules, wildcard imports, and task-scaffolding comments
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; Phase 9 §§16.1–16.7; Phase 10 §§17.1–17.5; ARCH-08; MODEL-03; COR-01; BOUND-01; PERF-01–09; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: Phase 11 §§18.1–18.5 and ARCH-09. The audit must distinguish cohesive numerical/schema files from orchestration god files and justify or merge tiny production modules without cosmetic churn.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error during Phase 7; textual fallback remains in use for that file. No current verification command fails.
Known failing tests: None. Phase 10 exit: formatting and denied-warning Clippy pass; Nextest passes 372/372 with 20 expected skips; standard all-feature Cargo tests pass all suites (270 library tests, 19 library skips; WSI 10/10 local plus one external skip); doc tests, no-default-features, and the exact WSI command pass.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/DECISIONS.md` record Phase 10 closure and Phase 11 entry.
Recent decisions: Format 0.3 result DTOs have six cohesive owners behind a small facade; public machine categories use closed enums and nested unknown fields are rejected; availability has one documented cross-family meaning; comparison orchestration and application-owned metadata are internal rather than supported crate-root API.
Unresolved technical questions: Broader validation calibration still needs multiple geometries, prevalences, null models, and seed families. Phase 11 must determine which remaining large files are cohesive declarations/numerical kernels and which still coordinate unrelated responsibilities.
Next three concrete actions: (1) measure production file and function sizes and inventory responsibilities for every Phase 11 named file; (2) search for wildcard imports, task-scaffolding comments, and one-function production modules; (3) add focused regression coverage before any responsibility-moving refactor
Next verification command: `wc -l src/spectra/structure_factor.rs src/api.rs src/api/stages.rs src/api/assembly.rs src/cli/multimodal/analyze.rs src/config.rs src/validation.rs src/prepost/deltas.rs src/output/writer.rs src/output/result_types.rs src/io/parquet.rs`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T07:39:17-04:00
