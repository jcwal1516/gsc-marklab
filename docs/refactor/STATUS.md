# Current Refactor Status

Plan version: 1.0
Current repository SHA: `9d01b048213e48fd5fa9643c9c878b02f6873b71`
Current branch: `refactor/audit-remediation`
Current phase: Phase 7 — Spectral and permutation optimization
Current workstream: PERF-08/ARCH-03 §§14.1–14.4 — reproduce mode-level permutation storage and make chunking/shell aggregation observable
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; BOUND-01; PERF-01–07; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: PERF-08 and Phase 7 §§14.1–14.10. ARCH-03 decomposition continues here. COR-01 remains open after honest smoke labeling; MODEL-02 remains open only for multimodal telemetry.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. Phase 6 exit: Nextest 349/349 with 16 expected skips; standard all-feature Cargo tests pass (250 library tests, 15 library skips, every integration suite); exact WSI integration passes 10/10 with one external-fixture skip; formatting, denied-warning Clippy, no-default-features, and doc tests pass.
Dirty files: Phase 6 closure and Phase 7 entry updates in decisions, findings, status, and performance records.
Recent decisions: The configured budget is divided into the existing base estimate and a remaining geometry allowance. One conservative index estimate plus the peak of the non-overlapping pair/residual plans must fit. Plans stop before adding an over-budget entry, and telemetry reports base plus peak geometry instead of the prior severe underestimate.
Unresolved technical questions: Confirm exactly which spectrum consumers need mode-level values versus shell curves, then define the smallest contiguous shell-level storage that preserves ERL, leave-one-out whitening, scalar outputs, deterministic order, and existing reference tolerances.
Next three concrete actions: (1) map all binary/probabilistic permutation matrices and `k_chunk_modes` consumers; (2) add red storage-shape, chunk-size equality, and serial/parallel differential contracts; (3) aggregate permutation modes into contiguous shell rows and reuse worker scratch
Next verification command: `rg -n 'Vec<Vec<f64>>|mode_powers|permutation_powers|k_chunk_modes|observed_power_for_modes' src/spectra src/api src/perf`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T05:13:56-04:00
