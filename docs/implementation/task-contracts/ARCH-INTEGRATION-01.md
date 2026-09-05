# ARCH-INTEGRATION-01 — Integrate the existing pathology suite

User authorization: 2026-09-04, implement the architecture review's recommended changes.
Parent scope: MASTER_PLAN §§6.4–6.12; PLAT-01, BACK-01, DATA-01, FND-04, WF-01,
WS-12, WS-13, WS-23, WS-25, WS-90, WS-92.

The first cohesive outcome makes the existing command catalog discoverable and executable from
one composed command tree. Subsequent outcomes make installed worker assets relocatable and
provision backend CI, then connect flexible multiplex inputs, shared application execution,
patient-level study composition, and result-policy reporting. Scientific validation and scale
promotion remain conditional on executed evidence and admitted data; no report finding is closed
merely by adding an API or ledger entry.

## Command discovery and routing milestone

- Compose the existing Clap-derived schemas into one tree used for help, validation, and dispatch.
- Preserve all existing command paths, arguments, defaults, feature gates, and scientific owners.
- Keep each typed argument decoder and existing execution handler; remove duplicate hand-maintained
  command-name dispatch lists from the binary entry point.
- Test missing root/family/nested help behavior red before implementation. Verify parsing failures,
  existing native workflows, and feature-dependent command availability afterward.
- Writable scope: binary command adapters, the hidden root CLI bridge, focused CLI tests, and
  implementation documentation. No numerical engine, result schema, dependency, or lockfile change.
- Canonical ownership: the composed command tree owns discovery/routing; existing method adapters
  continue owning typed decoding and invocation. Existing library/workflow/scientific owners remain.

Later outcomes receive concrete acceptance tests and recorded API/schema decisions before changing
their boundaries. Work remains in this checkout, with no push, deployment, external data access,
or new backend installation implied by implementation authorization.

## Command outcome evidence

- `cargo +1.96.0 test --locked --package marklab --test cli_discovery`: expected initial red
  (3 failed, 1 passed), then 4/4 passed. Composing the old 99-variant Bayesian parser exposed a
  debug-build stack overflow; LLDB identified `BayesCommand::augment_subcommands`. Eight flattened
  scientific command families remove that failure without changing command paths. An exact source
  comparison confirmed all 99 argument declarations are byte-identical to the baseline.
- `cargo +1.96.0 test --locked --package marklab --test cli --test cohort_energy_cli --test
  arbitrary_window_ipp_cli --test marked_prepost_dag_workflow`: 24/24 passed (17/2/1/4).
- `git diff --check` passed. Formatter, Clippy, feature matrices, full workspace, backend fits,
  packaging, and performance gates are reserved for the related stabilization checkpoint.

## Runtime delivery outcome

- Writable scope: shared `src/python_backend.rs` location owner; existing binary asset/runner
  adapters; `backend doctor`; its static worker; release archive configuration; focused runtime
  integration tests and installation documentation. DEC-0409 records the concrete public bridge.
- Explicit missing/relative runtime-root tests and the doctor test failed red before production
  changes. Admission must not silently fall back or require an interpreter on durable replay.
- The relocated-bundle test copies the real executable, exact lock/project metadata, and normal
  worker to a path containing spaces. It executes a real bounded fit using the existing environment,
  then replays byte-identically with a nonexistent interpreter and execution disabled, retaining
  exactly one ledger row. Existing worker validation remains authoritative.
- `cargo +1.96.0 test --locked --package marklab --test python_backend_runtime --test
  cli_discovery`: 9/9 passed after the initial 3/3 runtime red.
- `cargo +1.96.0 test --locked --package marklab --test bayes_normal_mean_cli --test
  topology_raster_morphology_cli --test durable_pymc_project`: 4/4 passed, including the original
  conjugate oracle, truthful nonconvergence, topology oracle, and exact durable replay.
- A local Unix archive smoke packaged the actual debug executable with 156 worker/control assets,
  extracted it to a new directory, and ran `backend doctor` successfully without an asset override.
  The final archive excludes environments and caches. This verifies archive layout, not a release
  optimization build or the Windows archive/runtime.
- Existing uv 0.7.17 dry-run command: `env UV_PROJECT_ENVIRONMENT=/Users/user/Bench/gsc-marklab/target/pymc-venv
  uv sync --project workers/python --locked --dry-run --python
  /Users/user/Bench/gsc-marklab/target/pymc-venv/bin/python`; passed with an up-to-date lock and no
  proposed changes. Ruby Psych parsed the release workflow; `git diff --check` passed.
- Python YAML parsing was attempted but `yaml` is not installed; Ruby Psych checked syntax instead.
  `actionlint` is unavailable locally, and hosted CI/cross-platform execution are not claimed.

## Backend CI outcome

The native job retains compilation, lint, documentation, feature, and WSI checks and runs native
library tests. The required backend job provisions the exact Python environment, checks admission,
runs the unfiltered workspace suite with bounded test concurrency, and executes Python regressions.
CI configuration is verified by existing workflow contracts, YAML parsing, the local locked uv dry
run, and direct execution of its Python test commands. A hosted green run requires a later authorized
push; no external run is started here. This declarative change adds no scientific behavior requiring
a separate numerical red test.

- Enabling the existing Python suite exposed three `ModuleNotFoundError: shapely` errors out of
  75 tests. DEC-0411 admits Shapely 2.1.2, already required by production adapters. uv regenerated
  the lock; comparison of all package name/version pairs confirmed no existing version changed.
  Synchronizing the existing project environment installed only Shapely.
- The three exact failed geometry tests then passed. Full commands
  `env PYTHONDONTWRITEBYTECODE=1 target/pymc-venv/bin/python -m unittest discover -s tests/python
  -p 'test_*.py'` and the same command with `-s workers/python` passed 75/75 and 1/1 respectively.
- `cargo +1.96.0 test --locked --package marklab --test workflow_contract` passed 7/7.
  Ruby Psych verified CI YAML and environment-install ordering before the complete nextest command;
  `git diff --check` passed. Hosted execution and Linux dependency-install capacity remain unverified.


## Multiplex study outcome — DEC-0412 / IC-0201

The immediate production caller is a complete multiplex protein panel-to-patient study. Writable
scope is the existing MarkTable and spatial-autocorrelation owners, the concrete library application
and its CLI/Python adapters, existing output-transaction visibility, release/client CI assets,
focused tests/examples and implementation documents. No new crate or production statistical engine
is introduced. Existing public modality/unit/error enums and result-format 0.3 remain unchanged.

The shared nullable assay table carries several named quantities, binary observations and nominal
codebooks with exact units, status, provenance and Cell IDs. Radius Moran/Geary share admitted
observed-cell graphs. Strict recipes freeze group/exchangeability, radius, selected family,
missingness, equal-slide patient reduction, seed/permutations and resource limits. Patient Max-T
uses the canonical cohort implementation. A required unavailable slide prevents inference; all
slides/patients and diagnostics remain visible. The existing policy owner supplies an experimental
or unsupported-for-claim ceiling from actual outcomes.

Durable slide, bounded collection and cohort nodes use existing scheduler/project owners and exact
float codecs. Partial runs stop at canonical slide boundaries; complete replay restores all slide
and patient outputs. More than 64 slides use concrete collections without weakening the durable
64-input limit. Atomic publication reuses the existing transaction and rejects nonempty output.
The CLI and Python client consume the same service and preserve exact scientific JSON.

Behavior-first coverage:

- Initial new assay/service tests failed to compile on the absent public entry points before their
  implementation. Independent small-graph Moran/Geary calculations, both weight policies,
  nullable observations and canonical patient Max-T parity then passed.
- A missing-observation hand oracle checks I = -1/148 and C = 75/148; exact and one-short directed
  edge/work budgets distinguish admission. Legacy scalar input coverage remains in place.
- Tests retain every patient/slide, reject invalid units, repeated source identities (including
  cross-slide aliases), contradictory groups, malformed input and resource overages; nominal
  labels cannot become numeric endpoints. A deserialized negative p-value cannot be published.
- Durable tests cover partial execution, exact all-hit replay, one-slide invalidation, 65-slide
  fan-in, library/CLI byte equality and process-boundary continuation. Twelve slides yield thirteen
  successful executions; sixty-five slides yield sixty-eight including two collections.
- The new f64 caller exposed underflow/overflow being mislabeled zero variance and a rounding
  residual for constant 0.1. The focused red distinguishes four constant/extreme cases. Exact
  constant detection and nonconstant numerical-failure classification now pass without changing
  ordinary formulas. Public legacy enum exhaustive-match coverage also passes.
- Focused final numerical command: `cargo +1.96.0 test --locked --package marklab
  --no-default-features --test multiplex_panel --test multiplex_study --test
  global_moran_typed_workflow --test global_moran_project_workflow`; all targets passed.

## Backend findings during combined verification — DEC-0413

The first combined nextest run stopped after 308 passes and one failure: moving the Bayesian
schema lost the explicit `anisotropic-gp-3d` command attribute. A focused discovery regression
failed red; restoring the attribute passed both discovery and the actual GP fit. A source comparison
then confirmed all 99 variant attributes and argument declarations match the baseline modulo
formatting. The original command-milestone evidence alone did not cover that attribute.

A second all-feature run used `--test-threads 2 --no-fail-fast` and exposed two failures. The SAR
fit reported 18,765 divergences for 2,000 transitions, while a direct unchanged retry passed. The
CellViT complementarity test copied a binary without its assets under the new relocation contract;
its helper now explicitly points to this checkout's runtime while retaining all assertions.

Installed PyMC 6.3.0 code confirms cumulative `divergences` versus per-transition `diverging`.
Twenty-five worker extraction expressions summed the wrong field. The regression extracts and
executes all 26 actual production expressions against synthetic flags/cumulative counters; the
true red had 50 failed subcases, and all expressions/cases now pass. The initial inventory test
incorrectly expected 25 total expressions; inspection found one already-correct worker and fixed
that test count before the meaningful red/green run. Thresholds and finite-result policy remain.

A two-process Python probe separately confirmed that `-I` ignored the declared hash seed. The
shared cleared-environment `-P -s -B` command now enforces repeatable hashes, no current/script or
user import paths, no inherited PYTHONPATH/PYTHONHOME and no bytecode writes. Its focused test
first failed on the ineffective old flags and now passes even with hostile ambient import paths.
Both actual backend runners retain their existing timeout, stream, schema and digest controls.
The extent to which hash ordering caused SAR fit variability is unproved.

The second nextest run was deliberately interrupted after these production fixes required a fresh
run. Its result was **869 passed, four failed, 28 skipped, 812 not run** (873 of 1,685 executed;
2 real failures plus 2 SIGINT interruptions; test phase 2,032.230 s). It is not a passing gate.
The corrected focused command `cargo +1.96.0 test --locked --package marklab --all-features
--test python_backend_runtime --test bayes_sar_fit_cli --test topology_raster_morphology_cli
--test cell_patch_complementarity_project_cli --test bayes_normal_mean_cli` passed 10/10.

## Python interchange outcome — DEC-0414

`clients/python/marklab_client.py` uses only the standard library and invokes the native study
service. The adapter bounds a selected matrix before sparse densification, carries observation
annotations separately, preserves nulls and observed zeros, requires explicit physical coordinates
and losslessly qualifies source-local Cell IDs by slide. Statistical formulas remain in Rust.

A separate locked test-only uv environment admits AnnData 0.12.4 and its scientific I/O dependencies.
The worker environment remains separate. Exact installation command:

```sh
UV_PROJECT_ENVIRONMENT=/Users/user/Bench/gsc-marklab/target/client-venv \
  uv sync --project clients/python --locked --group test \
  --python /Users/user/Bench/gsc-marklab/target/pymc-venv/bin/python
```

This installed 17 packages with Python 3.12.9. An initial attempt to pin AnnData 0.13.3 failed because
that release was unavailable (despite a stable-docs version label); the recorded decision uses the
verified published 0.12.4 release. The first real H5AD writer test exposed Pandas 3 nullable-string
index opt-in; fixture writing now uses AnnData's scoped `allow_write_nullable_strings` setting,
without changing adapter behavior. The actual H5AD round trip, malformed/materialization bounds,
and native process resume/replay tests pass 3/3. Required CI provisions this distinct environment.

## Parser and dependency evidence

The new fuzz target calls the actual bounded study JSON/application boundary. `cargo +nightly fuzz
check` passed. Its first invocation regenerated a stale fuzz lock and selected newer shared
packages. Precise thiserror and j2k downgrades initially failed on coupled transitive requirements.
Rather than retain that drift, Cargo reconciled the fuzz workspace offline from the existing root
lock: `cargo +1.96.0 update --manifest-path fuzz/Cargo.toml --workspace --offline`. Every shared
package/version now exists in the root lock; fuzz-only arbitrary/libfuzzer pins are unchanged.
The root Cargo lock has no diff. The final `cargo +nightly fuzz check` passed in 22.98 s. This is
compile evidence; execution evidence and remaining limits are recorded at the checkpoint below.


## Review coverage and retained limits

- Findings 1, 2 and 5: one nullable panel, reusable library application and complete durable
  slide-to-patient study now work together. Other existing binary-owned workflows have not been
  mechanically migrated, and no arbitrary heterogeneous workflow builder is claimed.
- Findings 3, 4 and 6: composed command discovery, explicit installed runtime assets, packaged
  workers/doctor and required provisioned CI are implemented. Local real fits and relocation pass;
  clean hosted CI and Windows/Linux backend admission need separately executed evidence.
- Findings 7 and 10: this study derives policy from its actual results and publishes the same
  limits exposed before execution. The capability guide covers supported workflow families.
  Retrofitting a common descriptor into every specialized result remains uncompleted.
- Finding 8: shared graph arithmetic, fixed admission bounds and a complete 60,000-cell synthetic
  workload are exercised. Dense GP/SPDE domains and other methods' scale limits remain; no general
  sparse solver, approximation, streaming input or million-cell inference claim is added.
- Finding 9: a thin Python client and actual AnnData/H5AD round trip feed the same application.
  SpatialData/OME-NGFF transforms, R bindings, zero-copy artifacts and broader interchange remain.
- Real pathology panel admission and independent scientific/biological calibration remain open.
  Synthetic panels and successful software tests do not replace these prerequisites or alter the
  previous diagnostic-only joint location–embedding pilot.


## Executed workload and package smoke

The optimized native build completed: `cargo +1.96.0 build --locked --release --features cli --bin
marklab` (11m 54s). The deterministic generator checks a complete six-patient/twelve-slide study at
72 and 60,000 total cells. Three cold and three replay samples per workload all pass graph/count,
exact scientific JSON and thirteen-ledger-row oracles. The larger workload's median wall times
are 0.86 s cold and 0.17 s replay, with maximum RSS below 50 MiB. CPU/I/O contention from concurrent
checks and warm OS caches are explicitly disclosed; these are diagnostic workload measurements,
not optimization or whole-slide capacity evidence. Exact commands, all samples, input/binary
hashes and limitations are in `docs/multiplex-study-measurements.md`.

The bounded fuzz execution command `cargo +nightly fuzz run multiplex_study
target/multiplex-fuzz-corpus -- -max_total_time=30 -max_len=65536 -timeout=10` completed 174,252 runs
in 31 s with no reported failure, seeded by the 9,039-byte shipped study. This short sanitizer run
is not exhaustive parser, numerical or maximum-size coverage. Mutated corpus and logs are ignored
local artifacts. Sanitizer/process RSS includes persistent fuzz state and is not the application's
per-study admission estimate.

Both exact project environments pass package consistency: `uv pip check --python
target/pymc-venv/bin/python` (74 packages) and the same command with `target/client-venv/bin/python`
(17 packages). The initial attempt `env UV_PROJECT_ENVIRONMENT=/Users/user/Bench/gsc-marklab/target/pymc-venv
uv pip check` selected host Python 3.14.4 instead and reported twelve host incompatibilities;
UV_PROJECT_ENVIRONMENT does not select the interpreter for that pip command. No host packages were
modified and those unrelated failures were not treated as project failures; the corrected commands
explicitly select the two project interpreters. Package consistency does not constitute a security
or transitive-version certification.


The final Unix archive-layout smoke packaged the actual optimized executable plus 167 curated
entries (17,806,475 compressed bytes), including workers/lock, standalone client, runnable
synthetic example and workflow/measurement guides. Extraction to a path containing spaces passed
`backend doctor` with no runtime-root override (Python 3.12, ten direct pins), then executed and
independently verified the shipped native study (twelve slides, six patients, thirteen ledger rows).
The archive contains no virtual environment or bytecode cache. This smoke uses the native default
CLI build; it is not the WSI-enabled release matrix, a Windows archive execution or a hosted run.
Local artifacts remain under `target/architecture-release-smoke-y2yn09_u`.


## Final major-checkpoint disposition

The final complete command `cargo +1.96.0 nextest run --locked --workspace --all-features
--test-threads 2 --no-fail-fast` ran all 1,701 selected tests in 2,816.825 s: **1,700 passed,
one failed, 28 existing skips** (12 slow passes). The only failure was the historical CI test's
blanket `python/tests` substring ban, which now matches `clients/python/tests`. All scientific,
CLI, runtime, durable, new multiplex and compatibility cases passed. Existing ignored performance,
scheduled calibration and external-fixture tests remain unchanged.

DEC-0414's clarification replaces the blanket ban with the obsolete root-package prohibition and
positive checks for both locked environments, install-before-test ordering, separate client
environment and required scientific/client tests. Production code and CI jobs were unchanged.
`cargo +1.96.0 test --locked --package marklab --all-features --test workflow_contract` then passed
7/7; focused warning-denied Clippy and final formatting also passed. The unfiltered 47-minute run
was not repeated after a test-only correction, and no fully green single unfiltered run is claimed.
The complete command outcomes, all remaining limits and historical failures are retained in the
validation ledger and STATUS rather than being relabeled as a passing full gate.


Final focused nextest command: `cargo +1.96.0 nextest run --locked --package marklab --all-features
--test workflow_contract`; **7/7 passed, zero skipped** (0.015 s test phase). This closes the known
CI assertion regression while retaining the exact complete-run disposition above. Final source,
whitespace and status review preserves the immutable master plan, original root manifest/lock,
all 233 tracker rows, and the verbatim roadmap history. The coherent changes are committed locally;
no push, hosted run, release publication or deployment is performed.
