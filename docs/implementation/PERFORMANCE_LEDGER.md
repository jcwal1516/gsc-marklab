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

## WS-B exit reproduction

Command: `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features -- --quick`

Environment and workload definitions are unchanged from the baseline above. The final run at `ac7da28da082f795381da0a03e8437fc4a774262` rebuilt the release profile in 2m00s. Gnuplot remained unavailable, so Criterion used Plotters.

| Workload | Final quick interval | Criterion comparison to prior local sample | Status |
|---|---:|---|---|
| Multiscale residual grid64 | 46.195–46.471 ms | no detected change, p = 0.08 | pass |
| CSV complete load, 10,000 cells | 9.0747–9.0989 ms | no detected change, p = 0.21 | pass |
| CSV decode/filter, 10,000 cells | 2.8807–2.9672 ms | no detected change, p = 0.08 | pass |
| Indexed nearest neighbor, 10,000 cells | 6.1204–6.1721 ms | no detected change, p = 0.50 | pass |
| Periodogram grid64 | 11.877–11.896 ms | no detected change, p = 0.68 | pass |
| Permutations n500/k16/B7 | 15.179–15.486 ms | no detected change, p = 0.56 | pass |
| ERL B7 | 15.256–15.304 ms | no detected change, p = 0.15 | pass |
| Structure factor n1000/k32 | 56.784–57.180 ms | no detected change, p = 0.86 | pass |

The first workspace attempt measured these same Criterion targets but was not a passing gate: Cargo subsequently forwarded `--quick` to the new child crates' implicit libtest benchmark harnesses, which rejected the flag. The red manifest regression and `bench = false` fix in `ac7da28` restrict the command to real Criterion targets; the table records the final passing rerun. These quick intervals are noisy smoke measurements, not an optimization claim.

The phase-boundary DHAT command passed 3/3 assertions (180 filtered) after a 7.12 s test-profile build, with the same 14 narrow-feature warnings. The WSI-enabled release synthetic smoke rebuilt in 1m07s and completed 12/12 scenarios and 120/120 replicates with zero failed replicates; it is validation smoke rather than a timing benchmark.
