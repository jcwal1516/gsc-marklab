# Performance ledger

Implementation base: `55fce12f10684a9081ca1f744f87d6f5feedcb24`

## Environment

- Date: 2026-08-22
- OS/hardware: macOS 26.5.2 arm64; Apple M4 Pro; 48 GiB memory; 12 logical CPUs
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`
- LLVM: 22.1.2
- Benchmark command features: all features; smoke profile; Criterion `--quick`; benchmark-specific thread policies unchanged

## Baseline workloads

| Workload | Command | Shape/output | Time | Memory/allocations | Status |
|---|---|---|---|---|---|
| Multiscale residual | `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --all-features -- --quick` | Existing smoke fixture/equivalent output | 45.703–45.746 ms | Criterion smoke only | pass |
| CSV complete load | same suite | Existing smoke fixture | 9.0462–9.2575 ms | Criterion smoke only | pass |
| CSV decode/filter | same suite | Existing smoke fixture | 2.7986–2.8557 ms | Criterion smoke only | pass |
| Indexed nearest neighbor | same suite | Existing smoke fixture | 6.2113–6.2404 ms | Criterion smoke only | pass |
| Periodogram | same suite | Existing smoke fixture | 11.657–11.775 ms | Criterion smoke only | pass |
| Permutation engine | same suite | Existing smoke fixture | 15.140–15.168 ms | Criterion smoke only | pass |
| ERL envelope | same suite | Existing smoke fixture | 15.300–15.313 ms | Criterion smoke only | pass |
| Structure factor | same suite | Existing smoke fixture | 56.196–56.479 ms | Criterion smoke only | pass |
| DHAT heap regression | `cargo +1.96.0 test --locked --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1` | Three existing allocation regressions; 180 other tests filtered | 11.944 s wall including build | Test assertions passed; 14 narrow-feature compile warnings | pass |

Criterion release compilation took 2m42s and the full command about 2m44s. Gnuplot was unavailable, so Criterion used Plotters; numeric timing output completed. These results reproduce the smoke workload only and do not replace scheduled full or million-row measurements.

No performance claim is approved until workload equivalence, output size, hardware, compiler, features, threads, and exact command are recorded.
