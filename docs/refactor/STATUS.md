# Current Refactor Status

Plan version: 1.0
Current repository SHA: `f061ec0a1deedd0b0ecd80a05e40533fb4efd4fe`
Current branch: `refactor/audit-remediation`
Current phase: Phase 13 — CI, documentation, and release readiness
Current workstream: §§20.1–20.5 — resolve the three remaining findings with evidence, audit CI/fuzz/docs/decision coverage, run dependency and release checks, and execute all end-to-end smoke paths
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; Phase 9 §§16.1–16.7; Phase 10 §§17.1–17.5; Phase 11 §§18.1–18.5; Phase 12 §§19.1–19.6; ARCH-05/07/08/09; MODEL-03; COR-01; BOUND-01; PERF-01–10; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: Phase 13 §§20.1–20.5; BOUND-05, DUP-08, and DUP-09 remain open and must be fixed or disproved with exact inspection/test evidence.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error during Phase 7; textual fallback remains in use for that file. No current verification command fails.
Known failing tests: None. Phase 12 exit: formatting and denied-warning Clippy pass; Nextest passes 372/372 with 21 expected skips; the standard suite passes 270 library tests with 20 skips and every integration; doc/no-default pass; exact WSI passes 10 local tests with one external skip.
Dirty files: `docs/refactor/PERFORMANCE_FINAL.md`, `STATUS.md`, `DECISIONS.md`, and `FINDINGS_MATRIX.md` form the verified Phase 12 closure and are ready to commit.
Recent decisions: Default spectral chunking preserves Phase 0 speed while reducing retained permutation storage 99%; tighter chunks are an explicit memory/time trade. Spatial-index and geometry-plan RSS increases are deliberate, bounded, and justified by 26–95% endpoint speedups. Complete multimodal before/after time is non-equivalent because final library users receive all configured work.
Unresolved technical questions: Broader validation calibration still needs multiple geometries, prevalences, null models, and seed families. Three registered findings remain open for Phase 13 evidence: BOUND-05, DUP-08, and DUP-09.
Next three concrete actions: (1) commit the verified Phase 12 performance closure; (2) inspect and resolve BOUND-05, DUP-08, and DUP-09; (3) audit Phase 13 CI/fuzz/documentation gaps before final dependency/package/end-to-end checks
Next verification command: `rg -n 'CurveTestResult|effective_length|diameter|max.*scale|wsi' src tests docs/refactor/FINDINGS_MATRIX.md`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T08:45:55-04:00
