# Final Performance Report

## Measurement identity

- Plan version: 1.0
- Production benchmark SHA: `99f6d78` (production code is unchanged through profiling-hygiene commit `f061ec0`)
- Baseline production SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`
- OS: macOS 26.5.2, Darwin 25.5.0, arm64
- CPU: Apple M4 Pro
- Memory: 51,539,607,552 bytes (48 GiB)
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`
- Profile: repository release profile (`opt-level=3`, thin LTO, one codegen unit)
- Small-workload method: one untimed warm-up followed by five timed samples; median reported
- Spectral reruns: fifteen samples for the initially flagged binary/continuous groups
- Spatial thread count: one
- Direct spectrum executor: 12-thread global Rayon pool, recorded explicitly by the chunk benchmark
- Peak memory: macOS `/usr/bin/time -l` maximum resident set size around the already-built benchmark/test process

Every timed closure verifies the same checksum as its warm-up. Edge counts,
pair counts, mode counts, shell counts, permutation counts, and returned
membership counts are recorded with the corresponding workload.

## Small-workload before/after comparison

Times are median milliseconds. Percent change is final relative to Phase 0;
negative is faster.

| Workload | Sizes | Phase 0 ms | Final ms | Change |
| --- | --- | --- | --- | --- |
| Nearest neighbor | 256 / 512 / 1,024 points | 0.167 / 0.578 / 1.904 | 0.088 / 0.279 / 0.407 | −47.3% / −51.7% / −78.6% |
| Radius graph, radius 1.5 | 256 / 512 / 1,024 | 0.080 / 0.283 / 0.992 | 0.065 / 0.234 / 0.455 | −18.5% / −17.2% / −54.2% |
| kNN graph, k=8 | 256 / 512 / 1,024 | 0.957 / 3.850 / 16.462 | 0.147 / 0.513 / 0.756 | −84.7% / −86.7% / −95.4% |
| Multimodal territories | 256 / 512 / 1,024 | 0.048 / 0.154 / 0.414 | 0.032 / 0.113 / 0.166 | −32.9% / −26.6% / −59.9% |
| Territory profiles, 16 territories | 256 / 512 / 1,024 | 0.044 / 0.050 / 0.048 | 0.023 / 0.065 / 0.080 | −46.7% / +29.7% / +67.2% |
| Structure factor, observed | 64 / 128 / 256 cells | 0.067 / 0.305 / 1.242 | 0.060 / 0.269 / 1.151 | −10.1% / −11.9% / −7.3% |
| CSV load | 256 / 512 / 1,024 rows | 0.177 / 0.453 / 1.372 | 0.183 / 0.422 / 0.701 | +3.2% / −6.8% / −48.9% |
| Parquet load | 256 / 512 / 1,024 rows | 0.283 / 0.544 / 1.436 | 0.246 / 0.450 / 0.698 | −13.0% / −17.4% / −51.4% |
| Complete marked analysis | 64 / 128 / 256 cells, 19 permutations | 0.795 / 2.340 / 7.636 | 0.612 / 2.200 / 8.030 | −23.0% / −6.0% / +5.2% |
| Complete multimodal analysis | 48 / 96 / 192 fused rows, 19 permutations | 0.386 / 1.040 / 3.096 | 0.520 / 1.278 / 3.445 | +34.8% / +22.9% / +11.3% |

The small 16-territory profile case is intentionally not used as the scaling
acceptance workload: it is dominated by index construction and remains near
the timer noise floor. The larger profile workload below grows both territory
count and returned memberships.

The raw complete-multimodal comparison is not equivalent work. At Phase 0 the
library engine returned only its primary result while the CLI separately
recomputed or added configured null sensitivities, residuals, and
extrapolation. The final application service computes every configured null
model plus reusable artifact data once for both library and CLI consumers.
The 11–35% increase therefore measures a materially larger, corrected
workload; it is not classified as a regression. Peak RSS grew only 7.5%.

## Reusable pair and territory plans

Observed-only Phase 0 scans are not directly comparable with final plan build
plus observed evaluation. The relevant endpoint workload is one observed plus
19 null label evaluations:

| Workload | Sizes | Estimated Phase 0 repeated scan ms | Final plan build + 20 evaluations ms | Change |
| --- | --- | --- | --- | --- |
| Mark-pair covariance | 256 / 512 / 1,024 | 0.680 / 2.380 / 9.260 | 0.504 / 1.295 / 2.597 | −25.9% / −45.6% / −72.0% |
| Residual territories | 256 / 512 / 1,024 | 0.620 / 3.180 / 12.500 | 0.190 / 0.966 / 2.043 | −69.4% / −69.6% / −83.7% |

Final retained-plan evaluation alone is 0.014/0.030/0.058 ms for one
mark-pair curve and 0.237/0.533/1.158 ms for 19 curves. Residual-territory
evaluation is 0.002/0.005/0.012 ms observed and
0.071/0.134/0.436 ms for 19 alternate labels. No distance or neighborhood
geometry is rebuilt per permutation.

## Indexed scaling acceptance

Fixed-density bounded-radius workloads used 1,024 / 2,048 / 4,096 / 8,192
points. Adjacent doubling ratios and output counts were:

| Workload | Final median ms | Ratios | Output size |
| --- | --- | --- | --- |
| Radius graph | 0.414 / 1.168 / 1.999 / 5.229 | 2.82× / 1.71× / 2.62× | 3,906 / 7,922 / 16,002 / 32,225 edges |
| Multimodal territories | 0.153 / 0.507 / 0.750 / 2.483 | 3.31× / 1.48× / 3.31× | 16 / 23 / 32 / 1 clusters on the deterministic truncated grid |
| Territory profiles | 0.172 / 0.632 / 0.838 / 2.574 | 3.67× / 1.33× / 3.07× | 5,342 / 11,644 / 23,198 / 48,372 returned memberships for 128 / 256 / 512 / 1,024 territories |

Every adjacent ratio is below the approximately 4× signature of the former
quadratic scans despite output size increasing with `n`.

Nearest-neighbor medians at 1,024 / 2,048 / 4,096 / 8,192 / 16,384 points
were 0.426 / 1.583 / 1.847 / 6.769 / 8.305 ms. The truncated-square fixture
alternates tree shape and therefore produces uneven adjacent ratios. More
stable 4×-input ratios are 4.34× (1,024→4,096), 4.28× (2,048→8,192), and
4.50× (4,096→16,384), far below the 16× growth of a quadratic scan.

## Spectrum time and memory

The Phase 0 `k_chunk_modes=64` value was ceremonial. It now imposes a real
memory cap, so the comparable product default is 256 modes per chunk.

| Field | Cells | Phase 0 ms | Final chunk 256 ms | Change |
| --- | --- | --- | --- | --- |
| Binary | 64 / 128 / 256 | 0.350 / 1.012 / 3.056 | 0.273 / 0.918 / 3.034 | −22.1% / −9.3% / −0.7% |
| Continuous | 64 / 128 / 256 | 0.473 / 1.262 / 4.153 | 0.382 / 1.304 / 4.244 | −19.2% / +3.3% / +2.2% |

A deliberately tighter 64-mode chunk costs 3.828 ms binary and 5.372 ms
continuous at 256 cells (about +25% and +29% relative to Phase 0). One full
chunk is faster at 2.592/3.905 ms but retains more phase scratch. Chunk size 1
is supported for extreme memory pressure and remains intentionally
pathological at 34.4/34.7 ms. This is a documented memory/time control, not
an unexplained regression.

At 256 cells, 796 modes, 8 shells, and 999 permutations:

- the former `B × modes` matrix is 6,361,632 bytes;
- the final `B × shells` matrix is 63,936 bytes, a 99.0% reduction;
- binary peak RSS is 9,027,584 bytes (8.61 MiB);
- continuous peak RSS is 8,978,432 bytes (8.56 MiB).

Bounded anisotropy exactly matches the dense reference and is 4.72×, 5.00×,
and 4.57× faster at 64/128/256 cells. All three DHAT contracts pass: observed
structure-factor evaluation, one permutation iteration, and raster refill
allocate no blocks after scratch/raster setup. The DHAT feature combination
also passes denied-warning Clippy.

## Peak resident memory

Grouped process RSS includes runtime/allocator overhead and is comparable only
within the same command shape.

| Group | Phase 0 MiB | Final MiB | Change | Explanation |
| --- | ---: | ---: | ---: | --- |
| Nearest neighbor | 6.86 | 7.72 | +12.5% | Deliberate linear spatial-index storage |
| Radius and kNN graphs | 7.78 | 8.34 | +7.2% | Shared index plus output edges |
| Pair correlation | 7.03 | 8.52 | +21.1% | Reusable pair/bin plan replaces repeated distance scans |
| Territories and profiles | 7.95 | 9.44 | +18.7% | Shared index and retained neighborhood plan |
| Observed spectrum | 7.17 | 7.30 | +1.7% | Within process noise |
| Binary spectrum permutations | 8.56 | 8.48 | −0.9% | Shell-level retained matrix |
| Continuous spectrum | 8.48 | 8.38 | −1.3% | Shell-level retained matrix |
| CSV/Parquet loading | 11.75 | 11.59 | −1.3% | Streaming builder plus index |
| Complete marked analysis | 9.03 | 9.45 | +4.7% | Additional typed result/telemetry with bounded spectral storage |
| Complete multimodal analysis | 8.13 | 8.73 | +7.5% | More configured analyses in one application run |
| Transactional output | Not recorded | 12.27 | N/A | Full Parquet/GeoJSON/figure/manifest transaction; analysis excluded |

The pair and territory memory increases are intentional and proportional to
points plus reachable pairs/neighborhoods, not all possible pairs. They buy
25–84% complete observed-plus-null time reductions and are checked against
the configured geometry budget. No production `result.clone()` or
`fused_cells.clone()` remains; metadata clones occur once at result/manifest
boundaries rather than per cell.

## Transactional output

Phase 0 did not record output writing, so no before/after percentage is
claimed. Fifteen-sample final medians for a precomputed marked result were
0.874 / 0.859 / 0.939 ms at 64 / 128 / 256 cells. The workload includes
result/QC/timing/manifest JSON, three curve Parquet files, residual territory
GeoJSON when available, figures, required-artifact validation, and atomic
same-filesystem rename. Analysis time is excluded. Group peak RSS was 12.27
MiB. Filesystem caching makes these sub-millisecond sizes inappropriate for a
scaling claim; the benchmark exists to detect future transaction regressions.

## Million-row ingestion

Exact command after the optimized benchmark binary was cached:

`/usr/bin/time -l env MARKLAB_BENCH_PROFILE=full cargo +1.96.0 bench --locked --all-features --bench pattern_load -- --quick`

| Stage | Phase 6 checkpoint | Final Criterion interval | Change assessment |
| --- | --- | --- | --- |
| Complete CSV load | 2.548–2.571 s | 2.569–2.613 s | Criterion: no statistically significant change |
| Decode and filter | 266.35–267.88 ms | 276.17 ms in the cached final run | About +3%; below investigation threshold |
| Indexed nearest neighbor | 2.268–2.275 s | 2.2786 s | About +0.4%; within noise |

The benchmark completed in 23.81 seconds with 451,018,752 bytes (430.13 MiB) peak
RSS versus 448,856,064 bytes (428.06 MiB) at the Phase 6 checkpoint (+0.5%).
Fixture generation remains streamed and outside the measured load stages.

One discarded run wrapped a fresh optimized compilation and reported 5.26 GiB
RSS. That value measures the compiler, not Marklab, and is excluded explicitly.

## Regression assessment

- No equivalent default workload regressed by more than 20%.
- The small observed-only pair/territory numbers are plan-build crossovers;
  complete observed-plus-null work is 26–84% faster.
- The tight 64-mode spectral regressions are the intended operational memory
  cap. Default 256-mode execution is within ±3.3% at the largest comparable
  small workload and retains 99% less permutation-matrix data.
- The complete multimodal run performs more required work than Phase 0 and is
  therefore not an equivalent regression comparison.
- Spatial index/plan memory growth is deliberate, bounded, and accompanied by
  subquadratic scaling. No unbounded phase cache or all-pairs matrix exists.

## Commands executed

- `cargo +1.96.0 test --release --locked --all-features --lib --no-run`
- `/usr/bin/time -l env MARKLAB_BASELINE_SAMPLES=5 <release-test-binary> <benchmark-filter> --ignored --nocapture --test-threads=1` for all baseline-compatible and Phase 6/7 groups
- Fifteen-sample reruns for binary/continuous spectrum and output transaction
- Binary and continuous 999-permutation spectrum memory probes
- `cargo +1.96.0 test --release --locked --features dhat-heap --lib dhat_ -- --nocapture --test-threads=1`
- `cargo +1.96.0 clippy --locked --features dhat-heap --all-targets -- -D warnings`
- The cached full million-row Criterion command above

All listed benchmark/test commands exited 0. Gnuplot was unavailable, so
Criterion used its Plotters backend; timing and statistical analysis were not
affected.
