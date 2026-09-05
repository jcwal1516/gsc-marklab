# Native migration measurements

Measured 2026-09-05 on an Apple M4 Pro, 48 GiB RAM, macOS 26.5.2. The native candidate performs
the existing training-only logistic fit, disjoint conformal calibration, prediction sets and grouped
coverage. The original Python worker and its lock are unchanged at baseline `f784302`. Rust uses
the existing 1.96.0 optimized release profile. No numerical dependency or unsafe code was added.

## Equivalent-work results

Each row has ten paired repetitions in alternating implementation order, one computational thread
per implementation, the same scientific work and precision, and output verification on every pair.
The speedup is the median of paired Python/Rust ratios; the interval is a paired bootstrap 95%
interval with 10,000 resamples. Raw samples, binary/source/lock/input identities, all reference
failures and separate OS resource reports are retained in
[`grouped_conformal_measurements.json`](implementation/audits/grouped_conformal_measurements.json).

| Workload | Cold Python / Rust median | Cold paired speedup [95% interval] | Warm Python / Rust median | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| Original 30 patients × 2 features | 280.246 / 34.309 ms | 8.09× [7.84, 8.32] | 0.797 / 0.100 ms | 7.59× [6.15, 9.72] |
| Noisy 300 × 8, replication seed 2 | 273.456 / 33.008 ms | 8.26× [8.17, 8.42] | 3.173 / 0.607 ms | 5.19× [4.60, 6.10] |
| Balanced null 3000 × 32 | 320.242 / 43.646 ms | 7.29× [7.09, 7.50] | 22.849 / 5.119 ms | 4.45× [4.23, 4.55] |

Cold execution includes CLI startup, CSV admission, a child process, computation and atomic file
publication. It uses fresh processes with warm filesystem caches, not a cache-evicted machine.
Warm execution includes parsing, computation and JSON serialization; native measurements use the
second call in each process and Python is imported/warmed before sampling. The Rust CLI executes
with external backends disabled and nonexistent Python/runtime paths. The whole application uses
the same scientific library measured by the warmed development example.

Separate Darwin maximum-RSS observations (Python / Rust) are 72,318,976 / 25,853,952 bytes,
73,269,248 / 25,952,256 bytes and 83,083,264 / 32,505,856 bytes respectively. These are single
OS-reported maxima, not a sum of simultaneously live parent/child memory and not a memory
confidence interval. They do not replace hard input, output, patient, feature or deadline limits.

## What the measurements establish

These successful synthetic cases pass the prespecified coefficient/probability tolerances and
exact identity, ordering, prediction-set and count checks. Startup savings do not conceal a warm
slowdown on these cases. The full noisy panel contains eleven additional cases where the unchanged
SciPy BFGS optimizer reports precision loss. Their failures are retained; they provide no successful
head-to-head comparison. No reference tolerance, work amount or success policy was relaxed.
The large balanced-null design has analytically zero coefficients: it exercises size, preparation,
calibration and reporting but does **not** establish difficult iterative-fitting capacity.

All eleven failed-reference cases subsequently produced native fits. Evaluating those parameters
with the frozen Python objective gives finite losses and maximum absolute gradients from
1.70e-13 to 7.59e-9, all below the original 1e-8 criterion. Python's preparation, corrected rank,
prediction sets and coverage also agree at those fixed parameters. This audit substitutes parameter
evaluation for the reference optimizer call in an isolated test module; it does not edit the worker
or turn failed Python optimizations into successful fits. These are additional gradient/downstream
checks, explicitly separate from independent optimizer parity and speedup evidence.

The Python small-case profile identifies logistic optimization as the dominant cost. A native
1000-repeat balanced-null samply profile collected 10,304 samples before the last optimization:
SHA-256 compression accounted for 36.53% of exclusive samples and memmove 22.02%. The native path
serialized the complete input matrix merely to hash it. A streamed, unambiguous semantic hash now
consumes borrowed values directly; a Python `struct.pack` golden digest verifies its encoding.
Training data are prepared once in contiguous storage, and objective/gradient scratch is reused.
The final-build comparisons above were rerun after that change and the operation-order correction below, after all task builds and workspace tests stopped.

A later hand oracle found that the legacy Rust probability validator multiplied raw features before
dividing by training SD. Native prediction now shares the corrected order: standardize, then multiply,
as Python does. Means [0,0], SD [100,100], coefficients [2,-2] and features [1e308,1e308]
produce probability 0.5 instead of spurious overflow. The regression failed before the correction
and passes afterward. Earlier candidate samples remain in the audit JSON; they are not substituted
for the corrected candidate's measurements.

No parallel-scaling, real pathology, cross-platform, cache-replay or universal speedup claim is
made. There is no pre-existing durable grouped-conformal command to benchmark. Other production
Python capabilities remain in scope and retain their current execution paths.

## Reproduce

Use the separately locked Python environment at `target/pymc-venv`, as in repository CI. Preserve
the baseline release binary from `f784302` before rebuilding the primary checkout; do not overwrite
or reset user changes to obtain it. The record contains its exact SHA-256. Fixture generation and
all build, profile and comparison commands are recorded in the audit JSON. The harness is
[`tests/python/benchmark_grouped_conformal.py`](../tests/python/benchmark_grouped_conformal.py).
Criterion retains the 30×2 and converged 300×8 fits with frozen-oracle assertions in the existing
benchmark/CI arrangement. Its scoped `--quick` smoke passes; that executability check overlapped
the two-thread workspace test run and preceded the final operation-order correction; it supplies no
additional release speedup claim. Corrected release comparisons and affected tests were rerun. The exact
measured release candidate is retained at `target/native-migration/conformal/candidate-release-marklab`.
The inventory, contracts and decision ledger determine promotion;
this report alone is not a migration completion or release certificate.

## Patient-OOF probability calibration

The second native workflow preserves raw-score Platt fitting on training OOF rows, all held-out
predictions, Brier/ECE/reliability/Wilson output, and both diagnostic regressions. Its one/two-parameter
Newton fits combine objective, gradient and Hessian work over contiguous columns. Calibration and
conformal preprocessing remain separate. Exact original CSV transport avoids intermediate float
round trips, and native output carries its own source identity. Python sources/lock remain unchanged.

Ten alternating paired repetitions on the same M4 Pro and one CPU thread give:

| Workload | Cold Python / Rust ms | Cold paired speedup [95% interval] | Warm Python / Rust ms | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| 16 patients | 276.033 / 33.735 | 8.22x [8.02, 8.40] | 0.697 / 0.063 | 11.17x [7.47, 14.94] |
| 1000, original decimal scores | legacy validation fails | unavailable | 2.217 / 0.555 | 3.74x [3.36, 4.61] |
| 10000, original decimal scores | legacy validation fails | unavailable | 14.454 / 5.441 | 2.67x [2.60, 2.70] |
| 1000, scores rounded to eighths | 266.442 / 33.730 | 7.90x [7.83, 7.97] | 1.975 / 0.496 | 3.71x [3.55, 4.56] |
| 10000, scores rounded to eighths | 307.253 / 41.203 | 7.39x [7.25, 7.63] | 13.073 / 4.923 | 2.66x [2.59, 2.68] |

Constant, completely separated and saturated-score cases also pass parity and both timing gates;
all eight warmed cases and six successful cold cases retain raw samples in
[`prediction_calibration_measurements.json`](implementation/audits/prediction_calibration_measurements.json).
The two original decimal-score cases succeed in the unchanged Python worker and native application,
but the old CLI rejects returned predictions. They do not establish a complete-workflow speedup.
The added eighth-score panel preserves labels and splits and isolates the legacy transport limitation;
it does not replace those failures. The initial cold harness omitted required --method and is retained
as a harness failure. Its correction changes neither implementation nor numerical tolerances.

Cold measurements cover fresh complete CLI processes with a warm filesystem. Warm measurements
cover the actual Python request service and native CSV/control service, including every regression
and serialization. Profiling the native service collected 5577 samples: SHA-256 compression accounts
for 23.11% exclusive samples and memmove 11.39%; no SIMD or parallel claim follows. OS memory
observations are separate one-shot maxima, not a concurrent process-tree sum. The corrected native
binary is retained at `target/native-migration/calibration/candidate-release-marklab`; commands,
identities, profiles and limitations are in the audit. There is no durable calibration interface to
replay, and these synthetic measurements establish no real pathology or cross-platform capacity.
