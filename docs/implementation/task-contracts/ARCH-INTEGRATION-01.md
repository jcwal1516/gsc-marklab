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

## Current outcome: command discovery and routing

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
