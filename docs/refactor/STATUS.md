# Current Refactor Status

Plan version: 1.0
Current repository SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`
Current branch: `refactor/audit-remediation`
Current phase: Phase 0 — Baseline, evidence, and reproducibility
Current workstream: Behavior-focused regression reproductions and performance-baseline coverage
Last completed requirement IDs: Phase 0 §§7.1–7.3; COR-01 through COR-07 reproduced by concrete source inspection
Requirements currently in progress: Phase 0 §§7.4–7.5; regression reproductions for COR-01–COR-07, MODEL-04, OUT-01, OUT-04/05, and OUT-06
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: None in the committed baseline. Expected-failure regression tests have not yet been added.
Dirty files: All files under `docs/refactor/` are uncommitted Phase 0 records: `MASTER_PLAN.md`, `STATUS.md`, `DECISIONS.md`, `FINDINGS_MATRIX.md`, `PERFORMANCE_BASELINE.md`, `BASELINE_VERIFICATION.md`, and `INVENTORY.md`
Recent decisions: Preserve the dirty `branch/spatial-phenotype-recovery` checkout untouched; use the clean linked worktree. Treat RUSTSEC-2026-0253 as a real transitive risk but not a currently triggerable Marklab path because the reachable WSI cache keys have non-panicking drops and no relevant `catch_unwind`; evaluate the patched dependency separately from baseline capture.
Unresolved technical questions: Precise result-0.3 availability representation, confounding sensitivity result ownership, spatial-index backend, baseline benchmark fixture strategy, and safe transitive `lru` upgrade path remain undecided pending tests and measurements
Next three concrete actions: (1) add ignored expected-failure regressions for the twelve Phase 0 defect cases and prove each fails for the intended reason; (2) add missing benchmark coverage with three-size scaling inputs and correctness checks; (3) run and record the complete Phase 0 benchmark baseline
Next verification command: `cargo +1.96.0 test --locked --all-features remediation_ -- --ignored --test-threads=1`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T22:04:11-04:00
