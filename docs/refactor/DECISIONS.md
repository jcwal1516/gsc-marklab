# Refactor Decisions

Architectural and scientific decisions are append-only entries. Superseded decisions remain in this file with a link to the replacement.

## 2026-08-21 — Phase 0 opened from a clean linked worktree

- Context: The original checkout was on `branch/spatial-phenotype-recovery` with modified and untracked user work, while the master plan requires a clean start from `main`.
- Decision: Preserve the original checkout untouched and create `refactor/audit-remediation` as a linked worktree from `main` at `a642fbcdd80b5baf784cd633b707dc0283a24d11`.
- Consequences: All remediation changes and verification records live in `/Users/user/Bench/marklab-refactor`. The pre-existing branch and its dirty files are not part of this refactor and must not be modified, stashed, or discarded.
- Status: Accepted.
