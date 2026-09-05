# Multiplex application workload evidence

Date: 2026-09-04. ARCH-INTEGRATION-01. These are synthetic software/resource measurements,
not biological calibration, estimator promotion or a whole-slide capacity claim.

## Workload and environment

- Apple M4 Pro, 48 GiB physical memory, macOS 26.5.2, native aarch64; Rust 1.96.0.
- Build: `cargo +1.96.0 build --locked --release --features cli --bin marklab` (default features
  retained; optimized, thin LTO, one codegen unit; system allocator). Build completed in 11m 54s.
- Binary SHA-256: `e43605f1dfcc01f45a25a5027e70ecc05c04a9536f40d277146fff6342279305`.
- Generator: [`examples/multiplex-study/generate.py`](../examples/multiplex-study/generate.py),
  fixed six patients, two slides each, two selected quantitative channels, nullable binary and
  categorical annotations, 1.1 µm radius, binary-symmetric weights, 99 patient permutations, seed 41.
- Two per-slide grids: 2×3 (72 total cells) and 50×100 (60,000 total cells). Each workload has
  three independent empty-project cold runs and one replay of each project to a fresh output.
- Complete timed flow: process startup, bounded JSON admission, spatial summaries or durable
  restoration, patient inference or restoration, project persistence and atomic report publication.
  Cold means an empty Marklab project; OS file caches were not flushed.

`/usr/bin/time -l` reports elapsed wall time and maximum resident set size (bytes on this platform).
The full integration suite and fuzz compilation were active during measurement. CPU/I/O contention,
first-process startup and filesystem caches affect timings; no exclusive-machine benchmark or
causal speedup claim is made. In particular, the first tiny cold run was substantially slower.

## Executed results

| Workload | Mode | Wall seconds, all three samples | Median seconds | Peak RSS range, MiB |
|---|---|---|---:|---:|
| 72 cells | Cold | 1.52, 0.70, 0.75 | 0.75 | 27.062–27.438 |
| 72 cells | Replay | 0.14, 0.10, 0.10 | 0.10 | 26.609–26.719 |
| 60,000 cells | Cold | 0.86, 0.87, 0.83 | 0.86 | 48.266–49.766 |
| 60,000 cells | Replay | 0.17, 0.17, 0.17 | 0.17 | 45.688–46.094 |

Every run passed the generator's independent grid-count oracle: a slide graph has
`4 * rows * columns - 2 * rows - 2 * columns` directed edges. The large workload has 19,700 edges
per selected channel/slide and 945,600 reserved statistic edge evaluations across the complete
study. Both sizes retain twelve slides, six patients, two slides per patient and available
complete-family inference. These count checks supplement the independent small-graph numerical
tests; they do not establish biological validity.

All six results per workload were byte-identical. Each cold run executed twelve slide nodes;
each replay restored all twelve. Every project retained thirteen successful execution records,
including the patient node, after replay. Recipe SHA-256 values:

- 72 cells, 9,039 bytes: `4bed477e67488d7e4f011b4abaa1aaab2ffb954191ce290447ac4939b2e7a456`.
- 60,000 cells, 3,950,119 bytes: `5299d382da122f39b64df216798e7793c3d2fcabd6bd0013d29b69d837417f14`.

Raw timing files, scientific outputs and `measurements.json` are retained locally under
`target/multiplex-profile`; they are generated, ignored artifacts rather than source data.

## Reproduce

Build with the command above. Generate the larger recipe into a new path:

```sh
python3 examples/multiplex-study/generate.py --rows 50 --columns 100 --out target/panel-60000.json
/usr/bin/time -l target/release/marklab study run \
  --recipe target/panel-60000.json --project target/panel-project-1 --out target/panel-cold-1
python3 examples/multiplex-study/generate.py --rows 50 --columns 100 \
  --verify target/panel-cold-1/result.json
/usr/bin/time -l target/release/marklab study run \
  --recipe target/panel-60000.json --project target/panel-project-1 --out target/panel-replay-1
cmp target/panel-cold-1/result.json target/panel-replay-1/result.json
```

Repeat with two more new project/output paths and verify thirteen ledger rows after every replay.
For the small workload use the shipped recipe and default verifier dimensions. The generator's
exclusive output creation prevents silently replacing a recorded corpus. Record actual hashes
when regenerating on a different Python/libm platform.

Real irregular tissue windows, dense neighbourhoods, selected-channel missingness, large panels,
millions of cells and alternative inferential engines require their own representative workloads.
The 16 MiB recipe ceiling and conservative memory/work admission remain authoritative. No
optimization was introduced or promoted from these timings.
