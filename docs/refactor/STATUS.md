# Current Refactor Status

Plan version: 1.0
Current repository SHA: `efb81872965efc7c64fd3878505dc2cc0468b2cc`
Current branch: `refactor/audit-remediation`
Current phase: Phase 7 — Spectral and permutation optimization
Current workstream: PERF-08 §§14.1–14.5 — finish bounded mode storage, compact mark-field reuse, and scratch-buffer verification
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; BOUND-01; PERF-01–07; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: PERF-08 and Phase 7 §§14.1, 14.5, and 14.7–14.10. Primary spectrum §§14.2–14.4 are implemented and verified; anisotropy still retains a mode matrix. COR-01 remains open after honest smoke labeling; MODEL-02 remains open only for multimodal telemetry.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 ignored remediation remains; other expected skips are manual performance/validation workloads. Phase 7 checkpoint: the all-feature library suite passes 254/254 with 15 expected skips; exact chunk-size/storage tests, formatting, denied-warning all-target Clippy, and no-default-features check pass. Phase 6 full-suite evidence remains Nextest 349/349 with 16 expected skips.
Dirty files: This status, findings matrix, and Phase 7 shell-storage decision are dirty to record verified commit `efb8187`.
Recent decisions: Primary binary, continuous, and stratified spectrum permutations store only contiguous shell rows. Mode power is produced in configured chunks, accumulated in original mode order, normalized once, and then consumed by matrix-native ERL and scalar summaries. This preserves exact output across chunk sizes while bounding mode scratch.
Unresolved technical questions: Anisotropy genuinely consumes directional mode values but should retain only `B × k_chunk_modes` at once and accumulate tensor summaries before discarding each chunk. Measure the CPU cost of regenerating deterministic permutations per chunk against the memory reduction.
Next three concrete actions: (1) add an anisotropy dense-reference/chunk-storage contract and replace its `B × modes` matrix; (2) introduce the smallest shared binary/continuous mark-field execution boundary without obscuring optimized binary subsets; (3) run DHAT, serial/parallel equality, spectrum timing, and RSS comparisons
Next verification command: `rg -n 'Vec<Vec<f64>>|permutation_powers|weighted_modes|k_chunk_modes' src/spectra/anisotropy.rs src/spectra/structure_factor src/perf`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T05:31:46-04:00
