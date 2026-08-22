# Current Refactor Status

Plan version: 1.0
Current repository SHA: `20803d38481ddfc0cb0f608db0a2bf67f225ac66`
Current branch: `refactor/audit-remediation`
Current phase: Phase 10 — Result model and public API cleanup
Current workstream: ARCH-08 / MODEL-03 §§17.1–17.5 — inventory remaining result cohesion, string statuses, availability semantics, and public re-exports
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; Phase 9 §§16.1–16.7; COR-01; BOUND-01; PERF-01–09; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: Phase 10 §§17.1–17.5, ARCH-08, and MODEL-03. Earlier schema work removed placeholders and split marked/multimodal territory semantics, but result modules, remaining free-string statuses, availability rules, and public re-exports require a fresh evidence audit.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error during Phase 7; textual fallback remains in use for that file. No current verification command fails.
Known failing tests: None. Phase 9 exit: Nextest passes 369/369 with 20 expected skips; standard all-feature Cargo tests pass all suites (270 library tests, 19 library skips; WSI 10/10 local plus one external skip); formatting, denied-warning Clippy, no-default-features, doc tests, and the exact WSI command pass. Both scheduled 1,000-replicate calibration tests pass when explicitly enabled.
Dirty files: `README.md`, `SPEC.md`, `docs/migration-0.2-to-0.3.md`, `docs/validation-methodology.md`, `docs/refactor/ALGORITHM_NAMING_AUDIT.md`, `docs/refactor/STATUS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/DECISIONS.md` record Phase 9 closure.
Recent decisions: Validation generators create inputs only; all observations come from production results/errors. Quick suites remain smoke. Scheduled marked and multimodal calibration require the 95% Wilson upper bound to remain at or below nominal 0.05. The multimodal null randomizes both source-section label fields.
Unresolved technical questions: Broader calibration still needs multiple geometries, prevalences, null models, and seed families. Phase 10 must determine which remaining free strings are public schema versus report prose and whether splitting the current result module materially improves ownership without a cosmetic rewrite.
Next three concrete actions: (1) inventory every result/status/availability producer and consumer; (2) inspect `lib.rs` re-exports and identify unsupported public orchestration; (3) add schema regressions before replacing remaining free-string statuses or moving result types
Next verification command: `rg -n 'pub status: String|class: String|status: "|class: "|pub use .*::\*|pub use' src/output src/lib.rs`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T07:14:41-04:00
