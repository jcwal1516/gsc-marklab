# Performance Baseline

## Baseline environment

- Plan version: 1.0
- Git SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`
- Branch: `refactor/audit-remediation`
- Timestamp: 2026-08-21T21:53:08-04:00
- OS: macOS 26.5.2 (build 25F84), Darwin 25.5.0, arm64
- CPU: Apple M4 Pro
- Memory: 51,539,607,552 bytes (48 GiB)
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`
- Compiler profile: To be recorded per benchmark; Criterion defaults are not assumed to represent every production workload
- Thread count: To be recorded per benchmark

## Methodology

For every required workload, record at least three representative input sizes where scaling is material, with point density, edge count, permutation count, thread count, repeated wall-time samples, peak memory, compiler profile, and exact command. Benchmarks must verify equivalent outputs before comparisons are treated as performance evidence.

## Baseline results

Baseline measurements have not yet been run. Results will be added for nearest-neighbor distance, radius graph, kNN graph, pair correlation, marked and multimodal territories, territory profiles, observed and permutation structure-factor paths, probabilistic-mark spectrum, CSV and Parquet loading, and complete marked and multimodal analyses.

## Known measurement limitations

No baseline performance claim has been made. Peak-memory tooling, stable fixture generators, and feasible workload ranges must be established before Phase 0 closes.
