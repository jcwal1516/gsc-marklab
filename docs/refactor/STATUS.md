# Current Refactor Status

Plan version: 1.0
Current repository SHA: `a8d38c5346370825176a32dc378ac2d318a43f2b`
Current branch: `refactor/audit-remediation`
Current phase: Phase 5 — Input, schema, and output architecture
Current workstream: OUT-01 / DUP-07 — one authoritative in-memory telemetry and manifest path
Last completed requirement IDs: Phase 0 §§7.1–7.5; Phase 1 §§8.1–8.4; Phase 2 §§9.1–9.7; Phase 3 §§10.1–10.5; Phase 4 §§11.1–11.8; Phase 5 §§12.1–12.4 and §12.10; DUP-05, OUT-04, OUT-05, OUT-06
Requirements currently in progress: Phase 5 §12.9 / OUT-01 / DUP-07. COR-01 remains open after honest smoke labeling; COR-03 persisted sensitivity reporting remains Phase 5.
Known failing commands: `lsp outline src/spectra/structure_factor.rs --project /Users/user/Bench/marklab-refactor` failed with a client capability error after the initial `src/lib.rs` outline succeeded; textual fallback is in use. All mandated baseline verification commands passed.
Known failing tests: Two ignored `remediation_*` tests represent later-phase findings: COR-01 engine calls and OUT-01 telemetry. The most recent full all-feature Nextest run passed 317/317 tests with 14 expected skips; the OUT-06 slice's focused CLI/unit suites and warnings-denied Clippy pass.
Dirty files: Refactor status, decisions, matrix, and reproduction ledger record the verified OUT-06 path validation.
Recent decisions: Batch manifest IDs are one trimmed normal path component. Both marked and multimodal batch flows use one resolver that rejects blank, absolute, dot, parent, slash/backslash, and existing symlink targets; marked jobs are all resolved before parallel work begins.
Unresolved technical questions: Decide whether output-write duration belongs only in manifest/artifact telemetry or in the immutable scientific result; result.json and timings.json must derive from one authoritative object and remain identical where they overlap.
Next three concrete actions: (1) run and enable the OUT-01 telemetry regression; (2) map marked/multimodal timing mutation and manifest construction; (3) define one telemetry owner and remove clone/mutate divergence
Next verification command: `cargo +1.96.0 test --locked --all-features remediation_result_and_timings_sidecar_use_the_same_telemetry -- --ignored --nocapture`
Performance baseline location: `docs/refactor/PERFORMANCE_BASELINE.md`
Last updated: 2026-08-22T02:53:53-04:00
