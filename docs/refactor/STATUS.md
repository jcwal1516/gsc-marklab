# Current Refactor Status

Plan version: 1.0
Current repository SHA: `8508671a180ae2e8a61351e452605012d5dcd577`
Current branch: `refactor/audit-remediation`
Current phase: Phase 2 — Critical correctness remediation
Current workstream: COR-02 true orientation-preserving two-dimensional rigid registration
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; DUP-01, DUP-02, and DUP-03
Requirements currently in progress: COR-02, followed by COR-03–COR-07 and MODEL-04
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Twelve ignored `remediation_*` tests still represent open findings: COR-01 engine calls, COR-02 rotation, COR-03 distinct nulls, COR-04 finite enrichment and JSON round-trip, COR-05 empty bins, COR-06 axes, COR-07 QC denominator, MODEL-04 component mode, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The Phase 1 result-boundary test now passes, but COR-04's producer remains intentionally open until Phase 2.
Dirty files: Phase transition records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` until the documentation commit
Recent decisions: Phase 1 standardized average-even medians, named reject-versus-ignore finite policies, sample/population variance denominators, finite ratios, endpoint-namespaced deterministic seeds, inclusive plus-one permutation p-values with explicit minimums, and a serde-traversal finite result boundary. Historical ad hoc permutation streams were not a supported public contract and changed deliberately.
Unresolved technical questions: Whether scale-plus-translation remains a supported transform after true rigid registration, the precise versioned metadata name for that transform, and whether Phase 2 result-schema corrections should be staged internally before the planned format 0.3 migration
Next three concrete actions: (1) expand the ignored COR-02 reproduction into the complete required rigid-transform regression matrix; (2) implement dependency-free orientation-preserving rigid least squares and route `Rigid` to it; (3) update transform metadata, CLI help, documentation, and migration notes before enabling the regression
Next verification command: `cargo +1.96.0 test --locked --all-features --lib registration::tests::remediation_rigid_registration_recovers_known_rotation -- --ignored --exact`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-21T22:48:31-04:00
