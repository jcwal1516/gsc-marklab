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
