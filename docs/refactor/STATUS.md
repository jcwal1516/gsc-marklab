# Current Refactor Status

Plan version: 1.0
Current repository SHA: `f47ab43443a8d3a08b4b2b17adcd782ff1a31489`
Current branch: `refactor/audit-remediation`
Current phase: Phase 9 — Rewrite validation so it validates production code
Current workstream: COR-01 §§16.2–16.5 — expand production scenarios and remove marked-suite manual flags, tautologies, and unconditional passing
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; BOUND-01; PERF-01–09; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: COR-01 and Phase 9 §§16.2–16.7. The original six multimodal scenarios now execute production analysis and the former ignored engine-call reproduction passes. Required negative/positive/edge-case breadth and marked-suite outcome synthesis remain open.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error during Phase 7; textual fallback remains in use for that file. The accidental two-filter `cargo test` invocation during Phase 9 was corrected with separate exact runs; no verification failure remains.
Known failing tests: None in the current Phase 9 slice. Expected skips remain manual performance workloads and one external WSI oracle. The focused multimodal smoke suite passes 5/5 and pre/post passes 13/13; formatting, denied-warning Clippy, and no-default-features pass.
Dirty files: `docs/refactor/STATUS.md`, `docs/refactor/FINDINGS_MATRIX.md`, and `docs/refactor/DECISIONS.md` record commit `f47ab43`.
Recent decisions: Multimodal smoke outcomes may only be derived from production result/run fields. Pre/post smoke uses the explicitly descriptive margin result and does not claim equivalence. Quick CI coverage is named smoke, reports 95% Wilson intervals and failed denominators, and is not calibration evidence.
Unresolved technical questions: Choose production-derived acceptance fields for the remaining registration-residual, unrelated-label, affine, sparse, and invalid-input scenarios; formal calibration needs larger scheduled replicate counts and justified nominal intervals.
Next three concrete actions: (1) add the remaining required multimodal negative, positive, and edge scenarios through the public engine; (2) replace marked manual status insertion, unconditional passing, and the many-foci tautology with production-derived checks; (3) separate marked smoke output denominators and confidence intervals from formal calibration claims
Next verification command: `rg -n 'push_unique_flag|passed\s*=\s*true|count\| count >= 0\.0|small_sample_type_i_limit' src/synthetic_smoke.rs`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T06:41:20-04:00
