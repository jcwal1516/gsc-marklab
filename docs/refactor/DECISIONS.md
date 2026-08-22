# Refactor Decisions

Architectural and scientific decisions are append-only entries. Superseded decisions remain in this file with a link to the replacement.

## 2026-08-21 — Phase 0 opened from a clean linked worktree

- Context: The original checkout was on `branch/spatial-phenotype-recovery` with modified and untracked user work, while the master plan requires a clean start from `main`.
- Decision: Preserve the original checkout untouched and create `refactor/audit-remediation` as a linked worktree from `main` at `a642fbcdd80b5baf784cd633b707dc0283a24d11`.
- Consequences: All remediation changes and verification records live in `/Users/user/Bench/marklab-refactor`. The pre-existing branch and its dirty files are not part of this refactor and must not be modified, stashed, or discarded.
- Status: Accepted.

## 2026-08-21 — Phase 0 closed with test-only reproduction and benchmark scaffolding

- Context: Phase 0 required an untouched verification baseline, concrete critical-defect evidence, twelve minimal reproductions, and measured scaling before production remediation.
- Decision: Close Phase 0 at production baseline `a642fbc` with the test/benchmark harness committed through `ba7dd9f`. Keep desired-contract regressions ignored until their owning remediation phases, but require explicit `--ignored` runs to demonstrate the defects.
- Evidence: The exit Nextest run passed 249 tests with 23 expected skips; the all-feature Cargo suite passed with 167 library tests, 19 ignored library tests, 17/2 integration pass/ignore for engine spectrum, 15/1 for CLI, and 10/1 for WSI. Formatting, warnings-denied Clippy, doc tests, no-default-features, dependency policy, Machete, and fuzz builds passed. All ten benchmark groups exited 0 and recorded three-size medians plus peak RSS.
- Consequences: Production behavior is unchanged. The only production-file edits are guarded by `cfg(test)`: a multimodal engine-call counter and the private baseline harness. The matrix now distinguishes confirmed defects from pending static audit items.
- Remaining risk: `cargo audit` still reports three allowed warnings, including the newly reviewed `lru 0.18.1` unsoundness advisory. The public Aperio/OpenSlide test remains a scheduled external-fixture check. Profile timing is below the useful noise floor and needs a larger Phase 6 workload.
- Status: Accepted; Phase 1 may begin.
