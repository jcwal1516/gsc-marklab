# Current Refactor Status

Plan version: 1.0
Current repository SHA: `efa10899a0ba8fea15ba5800cec52b5be5c1f509`
Current branch: `refactor/audit-remediation`
Current phase: Phase 8 — Multimodal model completion
Current workstream: MODEL-02 §15.4 — add authoritative multimodal stage telemetry to the application run and result
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.10; Phase 6 §§13.1–13.8; Phase 7 §§14.1–14.10; BOUND-01; PERF-01–08; COR-03; MODEL-01; DUP-05/07; OUT-01–06
Requirements currently in progress: Phase 8 §15.4 and MODEL-02 multimodal telemetry. Phase 8 §§15.1–15.3 and 15.5–15.7 were already completed during boundary/schema/spatial remediation and must be regression-checked at closure. COR-01 remains open for Phase 9.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Only the COR-01 remediation test remains ignored; the other expected skips are manual performance workloads and one external WSI oracle. Phase 7 exit: Nextest passes 358/358 with 19 skips; standard all-feature Cargo tests pass every unit/integration/doc suite (259 library tests, 18 library skips; WSI 10/10 local plus one external skip); formatting, denied-warning Clippy, no-default-features, doc tests, and three DHAT contracts pass. The DHAT feature-only command emits nine pre-existing unused test-counter warnings but exits 0.
Dirty files: Phase 7 closure updates in status, findings, decisions, and performance evidence.
Recent decisions: Structure-factor nulls retain shell rows, directional anisotropy retains only a bounded mode chunk, and both use reusable scratch. A local immutable marked-analysis context caches cell counts, prevalence, and geometry without placing an invalidatable cache in `Pattern`.
Unresolved technical questions: Define whether multimodal artifact-projection time belongs in the domain application telemetry or only in output transaction telemetry while ensuring result and sidecar histories remain identical.
Next three concrete actions: (1) inventory all multimodal application stages and current timing producers/consumers; (2) add a red contract requiring populated, ordered telemetry in the application run, result document, and CLI artifacts; (3) time each scientific stage once without moving output-writing work into domain code
Next verification command: `rg -n 'timings|TimingStage|Instant|timed' src/multimodal src/cli/multimodal src/output tests/multimodal_cli.rs`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T06:15:17-04:00
