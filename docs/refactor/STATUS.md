# Current Refactor Status

Plan version: 1.0
Current repository SHA: `a0dee6da38c70109fbd594ab7cead42ca7d8341b`
Current branch: `refactor/audit-remediation`
Current phase: Phase 11 — God-file and code-shape cleanup
Current workstream: ARCH-09 §§18.1–18.5 — decompose the mixed marked-stage and synthetic-validation workflows, review Parquet writer cohesion, and justify or merge ceremonial modules
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; Phase 9 §§16.1–16.7; Phase 10 §§17.1–17.5; ARCH-05/07/08; MODEL-03; COR-01; BOUND-01; PERF-01–09; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: Phase 11 §§18.1–18.5 and ARCH-09. `api/stages.rs` and `synthetic_smoke.rs` remain confirmed mixed-responsibility modules; Parquet and large numerical modules require a cohesion judgment rather than automatic splitting.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error during Phase 7; textual fallback remains in use for that file. No current verification command fails.
Known failing tests: None. Phase 10 exit: formatting and denied-warning Clippy pass; Nextest passes 372/372 with 20 expected skips; standard all-feature Cargo tests pass all suites (270 library tests, 19 library skips; WSI 10/10 local plus one external skip); doc tests, no-default-features, and the exact WSI command pass.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/DECISIONS.md` record ARCH-07 closure and the current Phase 11 workstream.
Recent decisions: The output writer owns transaction orchestration only; document semantics and analysis-family projections have explicit owners; shared validated JSON/timing emission is a narrow adapter. Configuration ownership remains split without public path changes.
Unresolved technical questions: Broader validation calibration still needs multiple geometries, prevalences, null models, and seed families. Phase 11 must determine which remaining large files are cohesive declarations/numerical kernels and which still coordinate unrelated responsibilities.
Next three concrete actions: (1) split mark-pair covariance, multiscale energy, residual-territory, and periodogram policy out of `api/stages.rs`; (2) separate marked and multimodal validation orchestration/result policy from shared calibration statistics; (3) review Parquet writers and tiny modules against their real schema/algorithm ownership
Next verification command: `rg -n 'stages::|super::stages' src/api src --glob '*.rs'`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T08:31:00-04:00
