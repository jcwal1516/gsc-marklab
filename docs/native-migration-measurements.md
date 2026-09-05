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

## Calibrated late fusion and shared-solver recheck

Late fusion preserves unstandardized probability/availability features, meta-only fitting, separate
Platt calibration, observed missingness scenarios and every fixed-model ablation. Python's large
balanced-null profile makes 19000 feature-array conversions and 18000 raw-probability calls. Native
execution uses borrowed modality slices and accumulates ablations in the patient loop. No ablation
or diagnostic was removed. The large native profile contains 5454 samples; SHA-256 compression
accounts for 45.78% exclusive samples and CSV record reading 9.85%.

Ten alternating paired repetitions, one computational thread per implementation:

| Workload | Cold Python / Rust ms | Cold paired speedup [95% interval] | Warm Python / Rust ms | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| 30 patients, 2 modalities | 277.443 / 33.851 | 7.98x [7.74, 8.24] | 1.336 / 0.114 | 11.34x [10.21, 14.30] |
| 300 patients, 4 modalities | 273.879 / 33.486 | 8.27x [7.95, 8.40] | 3.429 / 0.512 | 6.77x [5.88, 7.56] |
| 3000 patients, 16 modalities, balanced null | 337.999 / 40.367 | 8.36x [8.14, 8.53] | 54.280 / 5.478 | 9.93x [9.81, 10.21] |

The demanding non-null, missing-column and constant-calibration cases fail unchanged SciPy fitting.
Native recovery produces finite fits whose original Python gradients are <=6.10e-9, with downstream
agreement at fixed native parameters. Those checks are not independent optimizer parity or speedup
evidence. The constant case preserves the initial coefficient nullspace through an exact solution;
it does not identify a unique slope. The initial benchmark Path-import failure is retained separately.

Extracting the shared likelihood leaves grouped-conformal science unchanged. Its recheck gives cold
speedups of 8.36x / 8.08x / 7.06x on small / 300x8 / large balanced-null fixtures. The new actual
CSV/control-service warm speedups are 6.31x / 4.48x / 2.38x. That larger warm boundary includes
application parsing/source binding and is not directly compared with the earlier typed-spec timings.
All intervals exceed one; all old failed Python references remain recorded.

[`late_fusion_measurements.json`](implementation/audits/late_fusion_measurements.json) retains the
commands, inputs, source/binary/lock hashes, raw samples, separate RSS observations, profiles, failed
attempts and conformal recheck. The same single-host/synthetic/cold-filesystem limits above apply.
The three-workflow stabilization passes 1744 workspace tests at two-test concurrency (28 existing
skips) and all 38 native CI cases with Python execution disabled; final native release remains open.

## Fixed-mass entropic partial transport

The native dual-coordinate solver retains epsilon entropy, fixed total mass, capacity inequalities,
all dense plan entries and unmatched/objective summaries. It stops only when feasibility and a
primal-dual residual pass. This specializes the same convex objective and avoids the reference's
dense SLSQP subproblem. Python's 8x8 profile spends 8 of 13 ms inside compiled SLSQP; the native
16x16 application profile has 1592 samples, with SHA-256 compression at 31.66% exclusive, memmove
10.36% and CSV reader preparation 8.48%. Some system-math addresses remain unresolved. No speculative
SIMD or parallel implementation was added.

Ten alternating paired repetitions on the same M4 Pro, one computational thread:

| Workload | Cold Python / Rust ms | Cold paired speedup [95% interval] | Warm Python / Rust ms | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| forced | 284.176 / 34.529 | 8.13x [7.83, 8.41] | 0.489 / 0.124 | 3.76x [2.16, 5.85] |
| inactive | 282.451 / 34.614 | 8.29x [7.89, 8.57] | 0.704 / 0.121 | 5.65x [3.87, 7.72] |
| binding | 277.677 / 33.782 | 8.22x [8.12, 8.37] | 2.068 / 0.148 | 13.94x [11.97, 16.16] |
| balanced | 288.801 / 34.231 | 8.40x [8.31, 8.61] | 1.111 / 0.135 | 7.73x [6.01, 10.02] |
| representative | 294.554 / 34.180 | 8.65x [7.18, 8.84] | 11.621 / 0.182 | 63.60x [60.84, 66.21] |
| demanding | 10509.570 / 34.914 | 300.83x [298.36, 305.88] | 10181.037 / 0.332 | 30616.08x [30245.37, 32329.19] |
| maximum_null | 3583.711 / 42.915 | 83.75x [81.63, 84.62] | 3287.172 / 2.327 | 1407.79x [1386.20, 1429.07] |

All seven independent successful references pass complete plan/marginal/objective comparisons at
1e-6 absolute/relative, exact input identities/costs/order/counts, and native feasibility <=1e-8.
The zero-capacity Python fit fails with a positive directional derivative; it is retained as a
failure, while an independent analytical Gibbs formula verifies the native zero-support solution.
There is no head-to-head timing or independent fitted-Python parity claim for that failure.

Cold includes actual three-file admission, fresh CLI/child execution and atomic output on a warm
filesystem. Warm compares the original Python JSON worker and native CSV/source-binding application,
including complete science and serialization; file reads and process startup are excluded. Large
ratios describe these particular converged optimization workloads, not a general Rust speed ratio.
[`partial_transport_measurements.json`](implementation/audits/partial_transport_measurements.json)
retains every sample, interval, separate one-shot RSS observation, profile, source/input identity,
command and failure. Profiling/builds/tests finished before paired timings. Durable replay, parallel
scaling, real data and cross-platform capacity were not measured.

## Patient-grouped predictive stacking

All patient-held-out log densities, the complete simplex fit, every leave-one-patient refit and sensitivity range are retained. A single contiguous row-scaled density matrix is reused. Exact zero weights remain admitted; the native optimizer uses a bounded active-set simplex step rather than a softmax approximation.

Ten alternating paired repetitions, float64, one computational thread on the same M4 Pro:

| Workload | Cold Python / Rust ms | Cold paired speedup [95% interval] | Warm Python / Rust ms | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| symmetric | 287.354 / 34.684 | 8.21x [7.98, 8.42] | 3.905 / 0.090 | 39.67x [37.37, 43.95] |
| boundary | 314.979 / 35.125 | 9.00x [8.80, 9.12] | 8.901 / 0.150 | 60.47x [53.38, 62.74] |
| representative | 314.933 / 34.830 | 9.01x [8.85, 9.14] | 12.254 / 0.209 | 58.93x [52.02, 61.36] |
| demanding | 818.687 / 41.628 | 19.44x [19.39, 19.82] | 529.418 / 8.113 | 65.82x [64.64, 66.72] |
| maximum | 34973.567 / 300.261 | 118.08x [116.29, 120.40] | 35817.964 / 263.803 | 136.18x [134.78, 137.14] |
| identical | 265.078 / 30.694 | 8.65x [8.58, 8.85] | 6.533 / 0.146 | 44.26x [42.23, 47.59] |
| duplicate_models | 265.742 / 31.119 | 8.58x [7.91, 8.63] | 3.559 / 0.108 | 33.83x [32.39, 35.09] |
| offset | 267.246 / 31.173 | 8.50x [8.38, 8.66] | 3.506 / 0.098 | 37.59x [34.22, 39.69] |
| near_duplicate | 294.813 / 32.249 | 9.09x [8.67, 9.56] | 16.789 / 0.283 | 58.91x [56.30, 60.42] |

All nine frozen references pass direct mixture/objective/weight/refit comparisons at 1e-6 absolute/relative, with exact identities, order, count and boundary-weight checks. The maximum-workload native profile places 32.45% of exclusive samples in objective evaluation, 21.61% in fitting and 13.16% in triangular solves. These Python comparisons used the retained candidate before immutable implementation-digest reuse. A separate ten-pair native-before/after check preserves direct scientific parity for stacking and four earlier native workflows. The maximum stacking and demanding calibration timing intervals cross one; no additional speedup is claimed there. Smaller digest-bound applications improve by 2.03–3.06x. No expensive unchanged Python fits were repeated solely for this contained reuse change.

[`predictive_stacking_measurements.json`](implementation/audits/predictive_stacking_measurements.json) retains raw samples, uncertainty, failures, profiles, input/binary identities and memory observations. Cold includes fresh CLI/child execution and atomic output on a warm filesystem. Warm includes complete application/worker computation and serialization, excluding process startup. Separate RSS observations are not memory uncertainty estimates. These synthetic results establish no real-data, cross-platform, parallel-scaling or durable-replay claim.

## Context-gated mixture of experts

Gate training, availability masks, disjoint Platt calibration, OOD geometry and every test prediction remain in the complete workflow. The native gate uses a verified analytic gradient and bounded limited-memory BFGS. The demanding Python profile spends 32.3 of 33.3 profiled seconds in numerical gradient estimation; this reference implementation explains the large workload-specific speedups.

Ten alternating paired repetitions, float64, one computational thread on the same M4 Pro:

| Workload | Cold Python / Rust ms | Cold paired speedup [95% interval] | Warm Python / Rust ms | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| small | 291.197 / 33.836 | 8.82x [8.08, 9.09] | 11.599 / 0.078 | 154.81x [137.47, 161.70] |
| representative | 1397.192 / 33.377 | 41.82x [41.28, 42.46] | 1124.277 / 0.698 | 1608.93x [1601.67, 1616.51] |
| demanding | 22972.992 / 53.306 | 437.53x [263.00, 457.19] | 21982.776 / 7.834 | 2832.63x [2739.50, 2872.08] |
| maximum_features | 1214.163 / 34.309 | 35.51x [34.81, 36.52] | 898.990 / 0.324 | 2805.53x [2727.80, 2848.05] |

All four successful independent references pass. The demanding-case tolerances are 1e-4 for coefficients/gates and 1e-5 for probabilities/fitted scalar summaries, justified by the original finite-difference optimizer’s stopping precision and an independent analytic-gradient SciPy comparison (DEC-0423). Other cases retain 2e-5 fitted-parameter / 2e-6 downstream tolerances; identities, masks, counts and OOD decisions remain exact. The constant-context Python failure remains a failure and is not a timing comparison. A 3687-sample native profile places 21.26% in required input hashing, 18.20% in optimizer evaluation and 20.97% in unresolved system math. The demanding precision audit has a stale nested dense-BFGS display label; its owner recorded the fit after switching and rebuilding bounded L-BFGS. That label is not an algorithm identity.

[`mixture_of_experts_measurements.json`](implementation/audits/mixture_of_experts_measurements.json) retains raw samples, uncertainty, failures, profiles, input/binary identities and memory observations. Cold includes fresh CLI/child execution and atomic output on a warm filesystem. Warm includes complete application/worker computation and serialization, excluding process startup. Separate RSS observations are not memory uncertainty estimates. These synthetic results establish no real-data, cross-platform, parallel-scaling or durable-replay claim.

## Paired Gaussian pCCA EM

The first native demanding candidate passed parity but failed promotion: warm Python/native times
were 158.24/310.71 ms and the paired speedup interval was entirely below one. The profile placed
75.79% of 9262 exclusive samples in fitting and 18.84% in posterior scores. This confirmed slowdown
was retained, then corrected by computing the fixed training Gram statistic once and reusing it
for exact Gaussian EM sufficient moments and the likelihood trace. All final patient scores and
held-out cross-view predictions are still computed. SVD initialization acts directly on a scaled
cross matrix, retaining weak paired singular directions without squaring its condition number.

After that complete optimization, the same three frozen cases pass ten alternating paired
comparisons with unchanged covariance/reconstruction, identified-output and full-trace tolerances:

| Workload | Cold Python / Rust ms | Cold paired speedup [95% interval] | Warm Python / Rust ms | Warm paired speedup [95% interval] |
|---|---:|---:|---:|---:|
| small | 172.399 / 33.563 | 5.12x [4.93, 5.28] | 9.504 / 0.155 | 67.45x [51.26, 73.19] |
| representative | 235.392 / 36.448 | 6.43x [6.35, 6.46] | 45.982 / 1.081 | 41.82x [39.93, 45.53] |
| demanding | 367.460 / 72.680 | 4.91x [4.62, 5.05] | 181.982 / 11.317 | 16.00x [15.56, 16.69] |

The optimized 5427-sample profile now attributes 34.55% exclusively to fitting, 9.16% to memory
copies, 8.42% to required input hashing and 7.13% to the bounded posterior system. Profiling ran
separately from all paired measurements; its complete output also passes the frozen oracle.
[`pcca_em_measurements.json`](implementation/audits/pcca_em_measurements.json) retains both the failed
performance candidate and accepted comparison, raw samples, uncertainty, memory observations,
profiles and commands. The original Bayesian-NUTS pCCA worker is unchanged. Cold includes fresh
CLI/child execution and atomic output; warm includes complete JSON admission, EM, diagnostics and
serialization. There is no real-data, cross-platform, parallel-scaling or durable-replay claim.
