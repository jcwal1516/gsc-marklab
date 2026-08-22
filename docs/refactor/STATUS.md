# Current Refactor Status

Plan version: 1.0
Current repository SHA: `ba7dd9fcc661affa4f4cdb910a590b043eef0681`
Current branch: `refactor/audit-remediation`
Current phase: Phase 1 — Foundational shared utilities and invariants
Current workstream: Canonical statistics semantics, seed namespaces, and finite result boundary
Last completed requirement IDs: Phase 0 §§7.1–7.5; COR-01–COR-07, MODEL-04, OUT-01, OUT-04/05, OUT-06, and PERF-01–PERF-10 reproduced; performance baseline recorded
Requirements currently in progress: Phase 1 §§8.1–8.4; DUP-01, DUP-02, DUP-03, finite-value policy, touched-endpoint seed derivation, and explicit imports
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Twelve ignored `remediation_*` tests fail intentionally when run with `--ignored`: COR-01 engine calls, COR-02 rotation, COR-03 distinct nulls, COR-04 finite enrichment and JSON round-trip, COR-05 empty bins, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Exact evidence is in `docs/refactor/REGRESSION_REPRODUCTIONS.md`.
Dirty files: `docs/refactor/PERFORMANCE_BASELINE.md`, `docs/refactor/FINDINGS_MATRIX.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/STATUS.md` contain the Phase 0 closure and Phase 1 transition
Recent decisions: Phase 0 is closed at harness SHA `ba7dd9f`; desired-contract regressions remain ignored until fixed. The finite/statistical foundation will use concrete functions with explicit missing/non-finite and denominator semantics, not a generic statistics framework.
Unresolved technical questions: Exact equivalence among existing median definitions, which call sites intentionally ignore non-finite inputs, historical seed-output compatibility, and the narrowest authoritative serialized-float validation boundary must be resolved by call-site inspection and tests
Next three concrete actions: (1) map every existing statistic and permutation p-value helper to its actual semantics and callers; (2) add red tests for canonical statistics, safe ratios, finite validation, and domain-separated seeds; (3) implement the smallest common modules and migrate only semantically equivalent callers
Next verification command: `cargo +1.96.0 test --locked --all-features --lib common::stats -- --test-threads=1`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T22:28:22-04:00
