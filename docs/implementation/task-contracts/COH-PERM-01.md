# Task contract — COH-PERM-01 patient-level scalar permutation

Status: active

Date: 2026-08-24

Parent requirements: FND-06, COH-01, INF-01A, WS-31, WS-34.

## User outcome

`marklab cohort permutation` compares one prespecified finite scalar spatial endpoint per patient
between two declared groups without treating cells, patches, bins, or repeated rows as biological
replicates. It reports the signed group-mean difference, a studentized patient-level statistic, and
an inclusive-plus-one permutation p-value from deterministic whole-patient label shuffles.

## Accepted input

- One strict CSV table with columns `patient_id`, `group`, `endpoint`, and optional `block`.
- Exactly one row per non-empty bounded patient ID, exactly the two CLI-declared non-empty groups,
  at least two patients per group, and finite endpoint values.
- Empty block values mean one common exchangeability block. Otherwise labels are shuffled only
  within exact declared blocks, and at least one block must contain both groups.
- A positive bounded permutation count, one seed, one declared alternative, and distinct group
  labels. Patient identity is never inferred from paths, filenames, row repetition, or lower-level
  objects.

## Observable output

One pretty JSON file with a final newline records format/version, exact input path and design,
patient/block/group counts, group means, group-A-minus-group-B effect, studentized statistic,
inclusive-plus-one p-value, requested/attempted/completed permutation counts, seed, and alternative.
Every successful run completes every requested replicate; a non-finite/zero-standard-error
observed or permuted contrast is an error rather than a silently dropped replicate.

## Oracle and focused acceptance

1. A four-patient hand fixture matches independently calculated means, difference, Welch-style
   studentized statistic, and an extreme-tail p-value.
2. A deliberately slow test-only permutation implementation matches the production result for
   fixed blocked and unblocked fixtures, and repeated runs are byte deterministic.
3. Duplicate patients, undeclared groups, singleton groups, non-finite endpoints, fully confounded
   blocks, malformed rows, zero standard error, zero/excessive permutations, and oversized input
   fail before output publication.
4. Existing scalar p-value, deterministic seed/shuffle, CLI, and no-default contracts remain
   unchanged.

## Non-goals

No cell-level fallback, endpoint extraction from result files, paired/repeated/multisite designs,
hierarchical bootstrap, intervals, functional curves, Max-T, MMD/energy, equivalence,
noninferiority, causal claims, durable project integration, backend registry, external process, or
Rust port of an external inference engine. No performance claim is made.

## Completion

The focused core/oracle and CLI tests pass; affected CLI and no-default compatibility checks pass;
the final diff leaves interrupted PLAT-DUR-01 files untouched and records remaining cohort scope
truthfully.
