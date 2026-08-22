# Current Refactor Status

Plan version: 1.0
Current repository SHA: `b56cc60913eadae19c8e8f9aac529c2cb03179d0`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: Phase 3 §10.1 algorithm naming audit
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, and MODEL-04
Requirements currently in progress: SCI-01, SCI-02, SCI-03, SCI-04, and SCI-05 naming audit; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Every Phase 2 correctness reproduction is enabled and passing.
Dirty files: Phase transition records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: Component modes now resolve through an explicit plan. Pooled omits component results, Separate skips and suppresses pooled endpoints, Both returns both, and Auto selects Both only for multiple components with largest fraction below 0.80; every result records requested/resolved mode and reason.
Unresolved technical questions: Phase 3 must choose accurate replacement terms for the current MODWT/wavelet/DoG/Bartlett public surface and decide where MMR-specific interpretation belongs; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) create the algorithm naming audit across source, schema, CLI, README, and SPEC; (2) reproduce SCI-01 through SCI-05 with implementation-level evidence; (3) choose and document the rename-versus-real-algorithm path before changing public names
Next verification command: `rg -n -i 'modwt|wavelet|scalogram|difference.of.gaussian|\bdog\b|bartlett|bayesian|beta.binomial|global envelope|equivalence|validation|territor|spatial.index' src README.md SPEC.md examples tests`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T00:01:25-04:00
