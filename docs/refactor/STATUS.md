# Current Refactor Status

Plan version: 1.0
Current repository SHA: `a29885b75a76e6a9e49b8623bedec0cfead5c06a`
Current branch: `refactor/audit-remediation`
Current phase: Phase 4 — Application and boundary refactor
Current workstream: DUP-04 and ARCH-02 — consolidate enrichment execution, then introduce the smallest marked application run
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, MODEL-04, and SCI-01 through SCI-09
Requirements currently in progress: ARCH-02/03/06, BOUND-02, DUP-04, PERF-09/10; COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5. ARCH-01/04, BOUND-03/04, and DUP-06 are complete.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. The current all-feature run passed 310/310 tests with 15 expected skips.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, and `docs/refactor/FINDINGS_MATRIX.md` record the verified `a29885b` multimodal output-adapter move before their documentation commit.
Recent decisions: Multimodal output consumes the application run, moves its result into the result document, borrows sidecar projections, and validates sidecar floats. CLI no longer owns scientific artifact projection or clones a complete multimodal result.
Unresolved technical questions: Consolidate enrichment through one execution core without obscuring its permutation policy; define a marked run/output lifetime that preserves observability/intermediate behavior while eliminating the marked result clone.
Next three concrete actions: (1) add a shared enrichment-core differential test and consolidate the two wrappers; (2) inventory marked `AnalysisEngine` stage inputs/outputs and observability dependencies; (3) introduce the smallest `MarkedAnalysisRun` plus consuming output path
Next verification command: `cargo +1.96.0 test --locked --all-features neighborhood::tests::enrichment -- --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T01:47:26-04:00
