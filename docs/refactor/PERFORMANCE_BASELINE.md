# Performance Baseline

## Baseline identity

- Plan version: 1.0
- Production baseline SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`
- Benchmark-harness SHA: `ba7dd9fcc661affa4f4cdb910a590b043eef0681`
- Branch: `refactor/audit-remediation`
- Recorded: 2026-08-21
- OS: macOS 26.5.2 (build 25F84), Darwin 25.5.0, arm64
- CPU: Apple M4 Pro
- Memory: 51,539,607,552 bytes (48 GiB)
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`
- Compiler profile: release, optimized with the repository's release profile
- Thread count: 1
- Repeated samples: 5 timed samples after one untimed warm-up
- Point layout: deterministic unit-spaced grids at approximately one point per square unit
- Peak-memory measure: macOS `/usr/bin/time -l` maximum resident set size for each grouped benchmark process

The harness is test-only and does not expose private algorithms through the public library API. Each repeated operation must return the same checksum as its warm-up, so timing samples cannot silently measure different work.

## Exact execution pattern

The release test binary was built with:

`cargo +1.96.0 test --release --locked --all-features --lib --no-run`

The resulting binary was `target/release/deps/marklab-2ec00e9ec7e4aad3`. Each group was run as:

`/usr/bin/time -l env MARKLAB_BASELINE_SAMPLES=5 target/release/deps/marklab-2ec00e9ec7e4aad3 <filter> --ignored --nocapture --test-threads=1`

Filters were `baseline_perf_nearest_neighbor`, `baseline_perf_radius_and_knn_graph`, `baseline_perf_pair_correlation`, `baseline_perf_territories_and_profiles`, `baseline_perf_structure_factor_observed`, `baseline_perf_structure_factor_permutations`, `baseline_perf_probabilistic_spectrum`, `baseline_perf_csv_and_parquet_load`, `baseline_perf_complete_marked_analysis`, and `baseline_perf_complete_multimodal_analysis`. All final runs exited 0.

## Median wall time

Times are median milliseconds. Ratios compare each size with the preceding size.

| Workload | Sizes | Median ms | Doubling ratios | Output/workload details |
| --- | --- | --- | --- | --- |
| Nearest-neighbor distance | 256 / 512 / 1,024 points | 0.167 / 0.578 / 1.904 | 3.45× / 3.30× | Mean distance checksum identical at all sizes |
| Radius graph | 256 / 512 / 1,024 points | 0.080 / 0.283 / 0.992 | 3.53× / 3.51× | Radius 1.5; 930 / 1,913 / 3,906 undirected edges |
| kNN graph | 256 / 512 / 1,024 points | 0.957 / 3.850 / 16.462 | 4.02× / 4.28× | k=8; 1,062 / 2,103 / 4,166 normalized undirected edges |
| Pair correlation | 256 / 512 / 1,024 points | 0.034 / 0.119 / 0.463 | 3.55× / 3.89× | Bin width 1.0; maximum radius 5.0 |
| Marked residual territories | 256 / 512 / 1,024 points | 0.031 / 0.159 / 0.625 | 5.12× / 3.93× | Three scales, min z=0; 11 / 25 / 25 territories |
| Multimodal territories | 256 / 512 / 1,024 fused cells | 0.048 / 0.154 / 0.414 | 3.21× / 2.70× | eps=1.5; half the fixture is MMR-abnormal IHC |
| Territory profiles | 256 / 512 / 1,024 fused cells | 0.044 / 0.050 / 0.048 | 1.14× / 0.95× | 16 fixed territories; timings are near the measurement-noise floor |
| Structure factor, observed | 64 / 128 / 256 cells | 0.067 / 0.305 / 1.242 | 4.58× / 4.07× | 196 / 440 / 796 modes, 8 shells |
| Structure-factor permutations | 64 / 128 / 256 cells | 0.350 / 1.012 / 3.056 | 2.90× / 3.02× | 19 permutations, 8 shells |
| Probabilistic-mark spectrum | 64 / 128 / 256 cells | 0.473 / 1.262 / 4.153 | 2.67× / 3.29× | 19 permutations, 8 shells |
| CSV load | 256 / 512 / 1,024 rows | 0.177 / 0.453 / 1.372 | 2.55× / 3.03× | Includes filtering and quadratic nearest-neighbor finalization; fixture generation excluded |
| Parquet load | 256 / 512 / 1,024 rows | 0.283 / 0.544 / 1.436 | 1.92× / 2.64× | Includes filtering and quadratic nearest-neighbor finalization; fixture generation excluded |
| Complete marked analysis | 64 / 128 / 256 cells | 0.795 / 2.340 / 7.636 | 2.94× / 3.26× | 19 permutations, one thread |
| Complete multimodal analysis | 48 / 96 / 192 fused output cells | 0.386 / 1.040 / 3.096 | 2.70× / 2.98× | Equal H&E/IHC inputs, 19 permutations, one thread |

## Peak resident memory

These figures include the release test process, fixtures retained by the grouped test, and allocator/runtime overhead. They are suitable for before/after comparisons on the same machine and command, not as isolated algorithm allocation counts.

| Group | Maximum resident set size |
| --- | ---: |
| Nearest neighbor | 7,192,576 bytes (6.86 MiB) |
| Radius and kNN graphs | 8,159,232 bytes (7.78 MiB) |
| Pair correlation | 7,372,800 bytes (7.03 MiB) |
| Marked/multimodal territories and profiles | 8,339,456 bytes (7.95 MiB) |
| Observed structure factor | 7,520,256 bytes (7.17 MiB) |
| Structure-factor permutations | 8,978,432 bytes (8.56 MiB) |
| Probabilistic-mark spectrum | 8,896,512 bytes (8.48 MiB) |
| CSV and Parquet loading | 12,320,768 bytes (11.75 MiB) |
| Complete marked analysis | 9,469,952 bytes (9.03 MiB) |
| Complete multimodal analysis | 8,519,680 bytes (8.13 MiB) |

## Baseline conclusions

- The kNN graph is unambiguously quadratic at these sizes: both doublings are approximately 4×.
- Radius graph, pair correlation, nearest-neighbor distance, marked territories, and multimodal territories also exhibit strongly superlinear growth consistent with the inspected all-pairs implementations.
- CSV and Parquet loading inherit quadratic nearest-neighbor finalization, so decoder throughput cannot be interpreted independently from geometry at larger sizes.
- Spectral observed work grows with both cells and mode count; the measured mode counts increase from 196 to 796 across the three sizes.
- Permutation spectra are already material but these small baselines do not isolate allocation cost or mode-level matrix peak memory. Phase 7 must retain shell/mode counts and use DHAT or equivalent allocation evidence.
- Territory-profile timings are too small and noisy for a defensible scaling claim at the current radius and 16-territory fixture. A larger profile-specific workload is required before PERF-06 is closed.
- The multimodal “million-cell” claim is not exercised because the production nearest-neighbor, graph, pair, and territory paths are still quadratic. Running that workload now would be misleading and impractical.

## Known limitations

- Five samples are enough to establish the large algorithmic gaps above, but not to claim small percentage improvements.
- Peak RSS is grouped-process memory, not per-function allocation attribution.
- Fixed-density grids cover a representative bounded-radius case but not sparse, clustered, duplicate-coordinate, or adversarial distributions.
- Complete-run fixtures use 19 permutations so Phase 0 remains practical; final performance work must report production-relevant permutation counts.
- Baseline measurements were made after the test-only harness was committed. Production algorithm code remained identical to the base SHA.

## Phase 6 indexed-geometry checkpoint

This is a remediation checkpoint, not a rewrite of the immutable Phase 0
baseline above. It was measured at commit `3c4a255` on the same machine,
compiler profile, thread count, fixture generator, and five-sample method. The
release test binary was built with:

`cargo +1.96.0 test --release --locked --all-features --lib <filter> -- --ignored --nocapture --test-threads=1`

Steady-state medians were then recorded with `/usr/bin/time -l` around the
already-built test binary. Checksums and edge counts exactly match the baseline.

| Workload | Sizes | Phase 0 median ms | Indexed median ms | Indexed doubling ratios |
| --- | --- | --- | --- | --- |
| Nearest-neighbor distance | 256 / 512 / 1,024 | 0.167 / 0.578 / 1.904 | 0.096 / 0.283 / 0.427 | 2.94× / 1.51× |
| Radius graph, radius 1.5 | 256 / 512 / 1,024 | 0.080 / 0.283 / 0.992 | 0.109 / 0.311 / 0.565 | 2.85× / 1.82× |
| kNN graph, k=8 | 256 / 512 / 1,024 | 0.957 / 3.850 / 16.462 | 0.172 / 0.635 / 0.841 | 3.70× / 1.32× |
| Territory profiles, 16 territories | 256 / 512 / 1,024 | 0.044 / 0.050 / 0.048 | 0.043 / 0.081 / 0.097 | 1.90× / 1.21× |

The radius graph has a 36% small-input overhead at 256 points and 10% at 512,
then is 43% faster at 1,024. This is the expected build/query crossover and is
retained rather than hidden. The original profile fixture remains too small and
now exposes index-build overhead; a larger territory-count workload is still
required before PERF-06 closure.

The retained manual `phase6_perf_spatial_index_nearest_neighbor_scaling`
workload measured 1,024 / 2,048 / 4,096 / 8,192 / 16,384 points at 0.421 /
1.549 / 1.763 / 6.637 / 8.062 ms. Adjacent ratios are 3.68× / 1.14× / 3.76× /
1.21× because the deterministic truncated-square fixture alternates tree shape;
the 4×-input ratios are 4.19× and 4.57×, far below the 16× growth expected from
the old quadratic scan. Phase 6 final scaling will add representative
non-grid/clustered workloads and report this fixture effect explicitly.

Peak RSS for the comparable 256–1,024 groups changed from 6.86 to 7.70 MiB for
nearest neighbor and from 7.78 to 8.38 MiB for the combined radius/kNN process.
The index trades linear memory for subquadratic queries; larger-workload peak
memory and one-build application reuse remain Phase 6 acceptance work.

### Mark-pair covariance plan checkpoint

`MarkPairCovariancePlan` builds indexed source/target/bin assignments once and
then evaluates label vectors without geometry. At 256 / 512 / 1,024 points:

| Work | Median ms |
| --- | --- |
| Indexed plan build plus one observed evaluation | 0.682 / 1.676 / 2.313 |
| One evaluation of a retained plan | 0.031 / 0.056 / 0.109 |
| Nineteen label evaluations over one plan | 0.560 / 1.038 / 1.815 |

The Phase 0 observed-only brute scan was 0.034 / 0.119 / 0.463 ms, so indexed
plan construction has a substantial small-input crossover and must not be
presented as an observed-only speedup. For one observed plus 19 null curves,
the measured indexed totals are approximately 1.242 / 2.714 / 4.128 ms versus
an estimated 0.680 / 2.380 / 9.260 ms for 20 repeated Phase 0 scans. Thus the
plan is slower at 256, near the crossover at 512, and about 55% faster at 1,024;
the advantage grows with the configured permutation count. The retained
`phase6_perf_mark_pair_covariance_plan` workload reports build and evaluation
separately so future changes cannot hide either cost.

### Residual-territory and indexed radius-consumer checkpoint

At commit `968d014`, marked residual territories use a contiguous per-scale
offset/neighbor plan. A complete plan build plus observed evaluation took
0.145 / 0.955 / 1.590 ms at 256 / 512 / 1,024 points. With that plan retained,
one observed evaluation took 0.002 / 0.005 / 0.011 ms and 19 alternate label
evaluations took 0.077 / 0.131 / 0.423 ms. The resulting observed-plus-19-null
totals are approximately 0.222 / 1.086 / 2.013 ms. Repeating the Phase 0
brute-force detector 20 times would take approximately 0.620 / 3.180 / 12.500
ms, so the reusable plan reduces this fixture by about 64%, 66%, and 84%.
The observed output counts remain exactly 11 / 25 / 25, and the independent
oracle compares complete candidate values and selection order across three
label assignments.

The plan-build-only scaling remains output-sensitive because the broadest
physical scale grows with the analysis window; it is not claimed to be a
bounded-radius linear workload. Production stores only configuration-eligible
scales. An explicit geometry-storage budget guard remains required before
Phase 6 closure.

Larger fixed-density bounded-radius measurements use 1,024 / 2,048 / 4,096 /
8,192 points:

| Workload | Median ms | Doubling ratios | Output size |
| --- | --- | --- | --- |
| Radius graph, radius 1.5 | 0.439 / 1.257 / 1.975 / 5.342 | 2.87× / 1.57× / 2.71× | 3,906 / 7,922 / 16,002 / 32,225 edges |
| Multimodal territories, eps 1.5 | 0.173 / 0.531 / 0.719 / 2.315 | 3.06× / 1.35× / 3.22× | 16 / 23 / 32 / 1 territories on the truncated-grid fixture |
| Profiles, radius 4.0 total | 0.192 / 0.573 / 0.781 / 2.530 | 2.98× / 1.36× / 3.24× | 128 / 256 / 512 / 1,024 territories and 5,342 / 11,644 / 23,198 / 48,372 returned memberships |

Every adjacent ratio remains below the 4× signature of the prior quadratic
scans even though profile territory count and returned membership both grow
approximately with point count. The grouped radius-consumer process peaked at
14,270,464 bytes (13.61 MiB) RSS. The residual-plan-only process peaked at
9,388,032 bytes (8.95 MiB); the comparable combined territory/profile group
was 7.95 MiB in Phase 0, reflecting the deliberate tree/plan storage trade.
