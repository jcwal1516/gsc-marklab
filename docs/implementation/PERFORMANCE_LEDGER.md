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

## C-01 hierarchy construction

Commands:

```text
env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features --bench cohort_hierarchy -- --quick
env MARKLAB_BENCH_PROFILE=full cargo +1.96.0 bench --locked --workspace --all-features --bench cohort_hierarchy -- --quick
```

The fixture is built outside the timed closure. Each Criterion iteration clones the declarative input using `BatchSize::LargeInput`, constructs and validates a new hierarchy, and asserts exact patient/specimen/block/slide/cell and pair/repeated counts. `sample_size(10)` and cell-count throughput are declared. The smoke shape is 100 patients/specimens × 100 cells = 10,000 cells; the permitted full fallback is 10,000 patients/specimens × 100 cells = 1,000,000 cells. Both include one block and slide per specimen. This measures hierarchy construction/validation, not embedding storage or a 10-million-cell claim.

| Shape | Final quick interval | Throughput | Status |
|---|---:|---:|---|
| 10,000 cells / 100 specimens | 1.1645–1.1726 ms | 8.5282–8.5873 million cells/s | pass |
| 1,000,000 cells / 10,000 specimens | 147.13–147.27 ms | 6.7900–6.7969 million cells/s | pass |

Environment matches the ledger header. Gnuplot remained unavailable and Criterion used Plotters. The implementation uses shared ID storage, indexed vectors/maps, iterative parent traversal, and a role-matrix-bounded specimen → patient biological lineage. The benchmark verifies equivalent output but does not measure peak memory, serialization, persistence, remote I/O, or inferential workloads; no broader performance claim is made.

## C-04 cell-embedding storage workload

Date: 2026-08-23. The OS, Apple M4 Pro host, 48-GiB memory, 12 logical CPUs, Rust 1.96.0, Cargo 1.96.0, and LLVM 22.1.2 match the ledger environment above. The benchmark uses root features `csv,parquet`, Criterion 0.7 with 10 flat samples, a one-second warm-up, a ten-second requested measurement window, and one calling thread. Criterion extended collection to one iteration per sample because both workloads exceed the requested window. Gnuplot was unavailable, so Criterion used Plotters.

Commands:

```text
cargo +1.96.0 test --locked --features csv,parquet,dhat-heap --test cellvit_embedding_heap -- --exact embedding_10k_1280_peak
cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '10k_x_1280' --noplot
cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet --no-run
/usr/bin/time -l cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '1m_x_256' --noplot
```

The synthetic fixture, arithmetic order, random-access sequence, digest framing, and RSS ownership are frozen by DEC-0030/DEC-0031. Both profiles check row count, dimension, canonical logical digest, and numeric digest on every iteration. The 10,000 × 1,280 profile pins logical/numeric digests `df74ee3588f3c5dbf4fa81ffc285dd9f84daf8bb1101e7294fba6536785931de` / `0396c2d78ff98c7307e7dcf383d8579cf1a06c2501f9fc67c91a285308de08b4`; the 1,000,000 × 256 profile pins `d5dc753238330e9be60fdee94c6c24d7151f26660527689b15e8895e983c5633` / `133dc720d9775d8d2a3c4140a36529a750ab844acd6970eb6ca2ff86d8e7d49a`.

| Profile | Checked work | Final interval | Throughput | Memory | Status |
|---|---|---:|---:|---:|---|
| 10,000 × 1,280 | Source import/finalization, sequential table/QC workload, prepublished Arrow/Parquet scans and materialized round trips | 4.1653–4.1803 s | 3.0619–3.0730 million values/s | Criterion timing; DHAT below | pass |
| 1,000,000 × 256 | Sequential QC/means/sum-of-squares/norm, 4,096 random row accesses, 16 × 16 population covariance, 64 × 64 linear kernel | 10.446–10.488 s | 24.408–24.506 million values/s | 1,117,552,640-byte maximum RSS | pass |

The timed full command completed in 126.19 s wall time and reported no compilation after the separate locked `--no-run` build. Its maximum RSS is 41.6% of the frozen 2.5-GiB host threshold. An earlier cold invocation that compiled and linked thin-LTO code inside `/usr/bin/time` reached 4,904,665,088 bytes; the immediately repeated current-binary calibration reached 1,098,268,672 bytes. DEC-0031 treats the former as disclosed build-resource usage and requires the current-binary timed command for embedding-runtime RSS.

The exact DHAT gate completed in 137.51 s. A diagnostic run of the same test reported zero current tracked bytes, a 324,194,945-byte peak, and a 603,979,776-byte cap. Unlike Criterion, its measured path imports the source table, publishes fresh Arrow and Parquet objects into a disk-backed temporary store, validates both scans and materialized reads, then drops every operation-owned value before sampling heap statistics.

These are storage/import/integrity workload checks, not embedding inference, biological analysis, a scientific estimand, or an optimization claim. The full profile does not exercise source-bundle import or physical columnar round trips; the smoke and authorized-corpus reconciliation own those complementary dimensions. Ten-million-row out-of-core behavior remains planned and unmeasured. C-04 remains open until the authorized 32-bundle Rust reconciliation and phase-boundary gates pass.
