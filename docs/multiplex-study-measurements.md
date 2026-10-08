# Multiplex study: measured run times

These measurements show how long `marklab study run` takes and how much memory it uses
on synthetic data. They are software timings, not scientific validation, and they do not
show how large a real whole slide can be.

## Setup

- Machine: Apple M4 Pro, 48 GiB memory, macOS 26.5.2, Rust 1.96.0 (September 2026).
- Build: `cargo +1.96.0 build --locked --release --features cli --bin marklab` (release
  profile with thin LTO and one codegen unit, system allocator).
- Data: [`examples/multiplex-study/generate.py`](../examples/multiplex-study/generate.py)
  with six patients, two slides each, two numeric markers, a yes/no and a categorical
  annotation, a 1.1 µm neighbor radius, binary weights, 99 patient permutations and
  seed 41.
- Two sizes: a 2×3 grid per slide (72 cells in total) and a 50×100 grid per slide (60,000
  cells in total).
- For each size: three runs into empty projects ("cold") and, for each, one re-run that
  reuses the saved project ("replay"). Times cover the whole command, from start-up to the
  written report. The operating system's file cache was not cleared.

Times and peak memory come from `/usr/bin/time -l`. Other heavy jobs were running at the
same time, so treat the numbers as rough. The very first small run was noticeably slower
because of start-up effects.

## Results

| Workload | Mode | Wall time, three runs (s) | Median (s) | Peak memory (MiB) |
|---|---|---|---:|---:|
| 72 cells | Cold | 1.52, 0.70, 0.75 | 0.75 | 27.1–27.4 |
| 72 cells | Replay | 0.14, 0.10, 0.10 | 0.10 | 26.6–26.7 |
| 60,000 cells | Cold | 0.86, 0.87, 0.83 | 0.86 | 48.3–49.8 |
| 60,000 cells | Replay | 0.17, 0.17, 0.17 | 0.17 | 45.7–46.1 |

Checks on every run:

- The neighbor graph had the expected number of edges, `4 * rows * columns - 2 * rows -
  2 * columns` per slide (19,700 per marker and slide for the large grid).
- All twelve slides and six patients were present, and the patient-level test was
  available.
- All six results for each size were byte-identical. Cold runs computed all twelve slides;
  replays reused all twelve.

## Reproduce

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

Repeat with new project and output paths. For the small size, use the shipped recipe and
the verifier's default dimensions. The generator refuses to overwrite an existing file.
The recipes used here had these SHA-256 hashes; a different Python or math library may
produce slightly different files:

- 72 cells (9,039 bytes): `4bed477e67488d7e4f011b4abaa1aaab2ffb954191ce290447ac4939b2e7a456`
- 60,000 cells (3,950,119 bytes): `5299d382da122f39b64df216798e7793c3d2fcabd6bd0013d29b69d837417f14`

Real tissue outlines, dense neighborhoods, missing markers, large panels and millions of
cells need their own measurements. The recipe size and work limits described in
[multiplex studies](multiplex-study.md) still apply.
