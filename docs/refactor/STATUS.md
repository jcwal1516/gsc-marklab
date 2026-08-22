# Current Refactor Status

Plan version: 1.0
Current repository SHA: `5de830485b4abc22abc77ff54d832893bd018a1b`
Current branch: `refactor/audit-remediation`
Current phase: Phase 11 — God-file and code-shape cleanup
Current workstream: ARCH-07 / ARCH-09 §§18.1–18.5 — separate result-document semantics and analysis-family artifact generation from output transaction orchestration; continue the code-shape audit
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; Phase 9 §§16.1–16.7; Phase 10 §§17.1–17.5; ARCH-05; ARCH-08; MODEL-03; COR-01; BOUND-01; PERF-01–09; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: Phase 11 §§18.1–18.5, ARCH-07, and ARCH-09. The audit has confirmed remaining mixed output and validation ownership; cohesive numerical files will remain intact unless a real dependency boundary is found.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error during Phase 7; textual fallback remains in use for that file. No current verification command fails.
Known failing tests: None. Phase 10 exit: formatting and denied-warning Clippy pass; Nextest passes 372/372 with 20 expected skips; standard all-feature Cargo tests pass all suites (270 library tests, 19 library skips; WSI 10/10 local plus one external skip); doc tests, no-default-features, and the exact WSI command pass.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/DECISIONS.md` record ARCH-05 closure and the current Phase 11 workstream.
Recent decisions: Configuration retains its public paths while model, defaults, decoding/override merging, and validation have explicit owners. Numerical modules are not split merely for line count; the output and synthetic-validation workflows remain confirmed responsibility triggers.
Unresolved technical questions: Broader validation calibration still needs multiple geometries, prevalences, null models, and seed families. Phase 11 must determine which remaining large files are cohesive declarations/numerical kernels and which still coordinate unrelated responsibilities.
Next three concrete actions: (1) move `ResultDocument` parsing/validation to its document module; (2) move marked and multimodal core artifact generation to their family-owned output modules while leaving transaction commit in the writer; (3) verify output failure atomicity and manifest parity before closing ARCH-07
Next verification command: `cargo +1.96.0 test --locked --all-features output::tests`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T08:07:00-04:00
