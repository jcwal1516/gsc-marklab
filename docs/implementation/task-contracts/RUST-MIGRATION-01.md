# RUST-MIGRATION-01 — Native execution with correctness and performance gates

Status: active migration; native grouped-conformal and patient-OOF calibration milestones complete.

Authorized 2026-09-05: implement the user's Rust migration plan in the primary checkout.
Parent coverage: MASTER_PLAN §§6.4–6.5, 6.8–6.12; BACK-01, PLAT-01, WF-01 and each
migrated scientific contract. The master plan and existing scientific claim limits remain intact.

All current production Python capabilities are in scope. Python remains an independent, pinned
verification oracle. Existing Rust WSI/codecs retain ownership; C/C++ numerical wrappers are not
substitutes for Rust algorithms. Follow the dependency direction CLI → application → scientific
owner → numerical primitives. No general backend registry or speculative numerical framework.

## First complete workflow: grouped-conformal classification

Preserve EMB-CONFORMAL-01 / IC-0098: exact patient splits, training-only population standardization,
positive-L2 logistic likelihood with unpenalized intercept, corrected finite calibration rank,
inclusive binary prediction sets, group coverage and the existing claim ceiling. The native
implementation must retain 30–100000 patients, 2–128 features, 12/10/8 split minima, the training
variation cutoff 1e-14, the 1000-iteration / 1e-8 gradient optimizer criterion, 16 MiB result bound
and 1–3600 second timeout. Singular/nonfinite arithmetic and failed optimization remain errors.

Writable owners: `crates/marklab-bayes/src/grouped_conformal*` for admission and scientific flow;
its private logistic module for model arithmetic; `marklab-numerics` only for the consumed optimizer;
the existing conformal CLI adapter and a hidden schema-registered native worker route for killable
execution. Public native fit/provenance additions are recorded in DEC-0415. Legacy Python request
and response types/readers remain available. No other method switches backend in this milestone.

Start with the existing 30-patient fixture plus deterministic 300x8 and 3000x32 patient/feature
workloads. Include near-collinear training features and label-balanced noisy data. Freeze inputs,
reference source/lock hashes, controls, ordering and source revision before measurement.
Cross-solver tolerances, fixed before running the candidate: preprocessing 1e-12 absolute/relative,
coefficients 1e-6 absolute/relative, probabilities and threshold 2e-7 absolute. These account for
the existing 1e-8 gradient termination of a regularized optimization; they do not relax the exact
prediction-set, count, identity or error assertions. Native output self-consistency retains 1e-10.
Analytical balanced-label cases and gradient finite differences supply independent checks.

For every workload record Python profile, parity, cold and warm timings, raw paired repetitions
(ten, alternating order), and memory. A slower or inconclusive candidate is not promoted merely
because it is Rust. Preserve failures and investigate; do not retune the reference or relax its
optimizer success policy to manufacture a comparison. No claim about real-data scale follows.

## Subsequent dependency-ordered outcomes

1. Deterministic statistics, calibration/fusion, adapters/reports and bounded H5AD interchange.
2. Analytically checked Gaussian inference, then generalized/spatial Bayesian workflows.
3. Exact topology/geometry and registration, then neural/SBI workflows.
4. Remove production Python assets only once every production inventory row passes its gates.

Immediate calibration/fusion boundary review: IC-0097 fits smoothed Platt targets on raw OOF
scores, starting at the class-count log odds, with L-BFGS-B's 1e-14 relative-objective / 1e-10
gradient controls. Its held-out diagnostic regressions have an offset or 1e-8 ridge and do not
refit the calibrator. IC-0099 instead trains unstandardized probability/availability features,
then fits smoothed Platt calibration on clipped raw-probability logits from a separate split,
starting at [0,1] with BFGS / 1e-8 gradient controls. Missingness uses probability 0.5 plus a zero
availability indicator. These are different contracts even where their logistic objective matches;
sharing preprocessing, initialization, stopping controls or evaluation labels would be incorrect.

Use red–green focused tests and affected integration tests per cohesive milestone. Run broad gates
only at major checkpoints, update the implementation ledgers from executed evidence, and make
local milestone commits. No push, publication or external data access is implied. A blocked
capability stays unpromoted while independent work continues.

## Executed evidence

Evidence below is chronological; the final checkpoint defines the current state. The inventory
baseline is commit `f784302`; the overall migration remains incomplete.

- Python baseline: `env PYTHONDONTWRITEBYTECODE=1 target/pymc-venv/bin/python
  tests/python/benchmark_grouped_conformal.py`. Original 30-patient fixture succeeds; its profile
  is dominated by logistic/BFGS work. The prespecified 300x8 and 3000x32 noisy alternatives both
  fail the unchanged SciPy optimizer with precision loss. These failures are retained under
  `target/native-migration/conformal`; no successful head-to-head claim is possible for them.
- Both new native-fit and bounded-BFGS tests fail red on the absent entry points. The initial
  canonical tests waited for the release build's Cargo lock; the narrow alternate artifact
  directory `target/native-migration/focused` allowed development without changing the checkout.
  First green: native fit 5/5 (analytic half-probabilities, exact inclusive sets, test-label leakage,
  admission, deterministic ordering, frozen SciPy agreement); BFGS 3/3 (quadratic, failures, bounds).
- The unmodified optimized baseline was built with `cargo +1.96.0 build --locked --release
  --bin marklab` (7m11s), then retained at `target/native-migration/baseline/marklab` before
  any CLI routing change. At that initial build no workload had passed a performance promotion gate.

### Final native candidate evidence — 2026-09-05

The complete CSV-to-native-child-to-atomic-report candidate now passes 9/9 Bayesian differential,
analytical, leakage, degeneracy and ordering tests; 4/4 numerical tests; three internal gradient/hash/overflow
oracles; 6/6 grouped-conformal CLI tests; 5/5 command-discovery tests; 7/7 CI/benchmark contracts;
2/2 dependency-boundary contracts; and 3/3 process lifetime/stream-limit tests. The added existing-
behavior coverage includes near-collinearity at minimum admitted alpha and changing all held-out
features/calibration labels without changing training preparation or fitted parameters.

The final optimized CLI build (`cargo +1.96.0 build --locked --release --bin marklab`) completed in
8m12s after the operation-order correction. Ten paired final-build comparisons pass on the original 30x2, admitted 300x8 seed-2 and
3000x32 balanced-null workloads. Cold paired speedups are 8.09x, 8.26x and 7.29x; warmed speedups
are 7.59x, 5.19x and 4.45x. All corresponding paired bootstrap 95% intervals exceed one. Raw
samples, memory, hashes, commands and eleven unchanged-reference precision-loss failures are in
`audits/grouped_conformal_measurements.json`; interpretation and limits are in
[`native-migration-measurements.md`](../../native-migration-measurements.md). No independently converged
large noisy Python fit, real-data, parallel, cross-platform or durable-replay claim follows. All
eleven failed-reference cases produce native fits whose frozen Python objective gradients are
below 1e-8; downstream Python evaluation at those fixed parameters agrees. These are separate
gradient/downstream checks, not successful Python optimizations or speedup comparisons.

Formatting, all-target/all-feature and CLI-only warning-denied workspace Clippy, no-default
workspace compilation, strict workspace Rustdoc and all seventeen doc-test targets pass. Full
workspace Nextest at default concurrency stopped after 457 passes and one unchanged hierarchy's
180-second backend timeout, leaving 1266 selected cases unrun. The exact unchanged hierarchy test
passed in isolation in 117.78 s. `cargo +1.96.0 nextest run --locked --workspace --all-features
--test-threads 2` then passed all 1724 selected tests in 2849.705 s (12 slow; 28 existing skips).
That suite used the compiled snapshot before the final contained prediction-order correction and
native-CI contract addition. The newly failing overflow regression now passes, as do all nine domain
cases, six all-feature CLI cases and current all-feature workspace Clippy. The modified CI contract
also failed red and passed green; the exact native CI selection passes 19/19 with external backends
disabled and nonexistent Python/runtime paths. A fresh unfiltered 1725-test run is not claimed.

The scoped Criterion `--quick` smoke passed both frozen workloads in the existing bench profile.
Its initial build took 13m17s, including the full-debug CLI, and its timed smoke overlapped the
workspace suite before the final prediction-order correction. It is executability evidence, not
additional release speedup evidence; corrected release paired comparisons were run after all task
builds and workspace tests stopped. No global benchmark suite or cross-platform release is claimed.

`actionlint .github/workflows/ci.yml` exited 127 (`command not found`). The locked scientific
Python environment also lacks PyYAML. Ruby Psych safe loading validated YAML syntax/environment;
the CI contract and actual native test command pass locally. These do not replace actionlint or
hosted CI, which remain unverified.

Scoped offline package verification initially failed because a Bayesian source include crossed
into a sibling crate. The numerics-owned source constant and package-local fixtures corrected that
boundary. Reusing the existing same-version temporary registry then exposed stale cached package
contents. The fresh-artifact command `env CARGO_TARGET_DIR=target/native-migration/focused cargo
+1.96.0 package --locked -p marklab-numerics -p marklab-bayes --allow-dirty --offline` passed both
package builds. This is scoped offline verification, not full-workspace publication or registry
resolvability. The pre-existing package metadata warnings remain.

The master-plan digest, all 233 tracker rows, Python oracle/lock and Cargo lock are unchanged.
The initial inventory generator read working-tree callers; it now reads frozen `f784302` blobs so
ports cannot erase their original caller map. A first piped batch read stalled and was terminated;
a bounded file-backed stdin read regenerated all 154 unique sources and retained the original
conformal CLI caller. No remote data or services were accessed, and no publication has
been made. The coherent native milestone is authorized for a local commit; other production Python
capabilities are still pending.

## Patient-OOF calibration continuation — 2026-09-05

DEC-0416 completes the bounded native CSV-to-calibration-to-report workflow with separate scientific,
application and process owners. Frozen SciPy parity includes 16/1000/10000 patients, constant scores,
complete separation, saturated finite test scores and two declared eighth-score panels. Smoothed
raw-score Platt fitting and both diagnostic regressions retain their objectives, starts, 1000-step
ceiling and 1e-10 gradient / 1e-14 relative-objective criteria using an exact-Hessian Newton solver.
The finite-difference gradient/Hessian and expired-deadline units pass. Analytical nonidentifiability,
leakage, ordering, legacy readers, exact-score transport and resource/publication boundaries are tested.

Ten alternating paired repetitions yield 2.66–14.64x warm speedups on all eight cases and 7.39–8.24x
cold speedups on six successful pairs of complete workflows; every corresponding bootstrap lower
bound exceeds one. The original 1000/10000-patient decimal cases fail unchanged legacy CLI result
validation, while their frozen worker and native application pass. Retain those two failures without
cold speedup claims. The eighth-score panel changes neither labels/splits nor the required work and
was declared to isolate that legacy transport limitation. The initial harness omitted --method;
those failed attempts and their valid warm samples remain separately recorded. A development-only
typed-spec JSON driver changed raw score bits and was replaced by actual native CSV transport.

The ordinary milestone uses focused tests, scoped Clippy, formatting and explicit performance gates.
It does not repeat the prior 47-minute workspace suite or claim hosted CI, parallel scaling, real-data
admission, durable replay or final native release. Evidence: `audits/prediction_calibration_measurements.json`.
Next production flow: late fusion, preserving its distinct score transformation, initialization,
BFGS gradient criterion, missingness representation and ablation estimand.
