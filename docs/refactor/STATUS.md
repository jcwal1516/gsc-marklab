# Current Refactor Status

Plan version: 1.0
Current repository SHA: `8ba7c9753246b189e49b680efd1537b343f61d55`
Current branch: `refactor/audit-remediation`
Current phase: Phase 9 — Rewrite validation so it validates production code
Current workstream: COR-01 §§16.1–16.3 — replace directly synthesized multimodal outcomes with production-pipeline scenarios
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; Phase 8 §§15.1–15.7; BOUND-01; PERF-01–09; COR-03; MODEL-01/02; DUP-05/07; OUT-01–06
Requirements currently in progress: COR-01 and Phase 9 §§16.1–16.7. The ignored Phase 0 reproduction still proves the multimodal synthetic smoke bypass and must become a passing production-call contract before Phase 9 closes.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 remediation test remains ignored; other expected skips are manual performance workloads and one external WSI oracle. Phase 8 exit: Nextest passes 359/359 with 19 skips; standard all-feature Cargo tests pass every suite (260 library tests, 18 library skips; WSI 10/10 local plus one external skip); formatting, denied-warning Clippy, no-default-features, and doc tests pass.
Dirty files: Phase 8 closure updates in status, findings, and decisions.
Recent decisions: Multimodal telemetry is application-owned and sequential (`cpu_threads = 1`). Registration residual/extrapolation preparation is the `artifact_projections` stage; filesystem serialization remains output-transaction work and is intentionally excluded. Every configured null model gets a distinct timing stage, and result/timings sidecar serialize one vector.
Unresolved technical questions: Determine the smallest honest production scenarios and acceptance outputs for the existing synthetic smoke cases; formal calibration replicate counts and confidence intervals remain distinct from quick CI smoke coverage.
Next three concrete actions: (1) inspect every direct boolean/status/pass synthesis in multimodal and marked smoke generators; (2) enable the ignored public-engine call regression and replace one multimodal scenario with real generated inputs; (3) derive scenario outcomes only from production result fields and report failed replicates explicitly
Next verification command: `rg -n 'detected|false_positive|below_registration_resolution|equivalent|passed\s*=|passed:' src/synthetic_smoke`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T06:25:37-04:00
