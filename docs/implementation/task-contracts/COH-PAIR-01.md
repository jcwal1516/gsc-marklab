# Task contract — COH-PAIR-01 paired patient scalar sign-flip

Status: complete

Date: 2026-08-24

Parent requirements: FND-06, COH-01, INF-01A, WS-31, WS-34.

## User outcome

`marklab cohort paired-permutation` compares one prespecified finite scalar endpoint under two
declared conditions for each complete patient pair. It reports the condition-B-minus-condition-A
mean difference, a studentized paired statistic, and an inclusive-plus-one p-value from
deterministic whole-patient sign flips.

## Accepted input

- One strict CSV table with columns `patient_id`, `condition`, and `endpoint`.
- Exactly one row for each declared condition per non-empty bounded patient ID, at least two
  complete pairs, finite endpoints, distinct non-empty condition labels, a positive bounded
  permutation count, one seed, and one declared alternative.
- Missing, duplicate, or undeclared condition rows are errors. Rows, cells, patches, and repeated
  observations never become independent replicates.

## Observable output and oracle

- One failure-atomically published pretty JSON file with a final newline records the exact input,
  paired design/unit, condition labels, pair count, condition means, condition-B-minus-condition-A
  mean difference, studentized statistic, inclusive-plus-one p-value, requested/attempted/completed
  counts, seed, and alternative.
- A four-patient hand fixture matches exact means, paired differences, and studentization. A slow
  test-only sign-flip implementation matches deterministic production p-values. Repeated CLI runs
  are byte-identical; incomplete or duplicate pairs and zero standard error publish no output.

## Non-goals

No unpaired fallback, repeated-measures residual permutation, blocks beyond the patient pair,
curves, Max-T, bootstrap, intervals, durable project, backend registry, or biological/causal claim.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed five unit tests and both slow
  differential reference tests.
- `cargo +1.96.0 test --locked --all-features --test cohort_paired_permutation_cli` passed the
  hand-oracle and byte-determinism workflow.
