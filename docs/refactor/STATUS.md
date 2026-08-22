# Current Refactor Status

Plan version: 1.0
Current repository SHA: `b56cc60913eadae19c8e8f9aac529c2cb03179d0`
Current branch: `refactor/audit-remediation`
Current phase: Phase 3 — Scientific naming and algorithm integrity
Current workstream: SCI-01/SCI-02/SCI-03 accurate multiscale-residual terminology
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; DUP-01, DUP-02, DUP-03, COR-02, COR-03 execution/conclusion semantics, COR-04, COR-05, COR-06, COR-07, and MODEL-04
Requirements currently in progress: SCI-01, SCI-02, SCI-03, SCI-04, and SCI-05 naming audit; COR-03's versioned sensitivity-result field remains staged for Phase 5
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Four ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls, OUT-01 telemetry, OUT-04/05 optional absence, and OUT-06 traversal. Every Phase 2 correctness reproduction is enabled and passing.
Dirty files: Phase transition records in `docs/refactor/STATUS.md`, `docs/refactor/DECISIONS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/REGRESSION_REPRODUCTIONS.md` until the documentation commit
Recent decisions: The audit confirms the default rename path: multiscale residual/scale-energy terms replace false wavelet/MODWT/scalogram/DoG names; the raster diagnostic becomes a Hann-tapered periodogram with real radial shells; generic interpretation becomes neutral. Four additional naming defects are registered as SCI-06 through SCI-09.
Unresolved technical questions: Exact 0.3 field names and artifact filenames for the multiscale residual rename must remain cohesive; COR-03's versioned sensitivity-result field remains Phase 5 work
Next three concrete actions: (1) add tests for the renamed multiscale result/config/serde contract; (2) rename implementation, output, artifacts, reports, and docs together; (3) remove constant-zero territory QC overlap through typed absence and run the all-feature suite
Next verification command: `cargo +1.96.0 test --locked --all-features --test api_contract -- --test-threads=1`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T00:06:29-04:00
