# Task contract — COH-MAXT-01 patient-level Max-T permutation

Status: complete

Date: 2026-08-24

Parent requirements: FND-06, COH-01, INF-01C, WS-31, WS-34.

## User outcome

`marklab cohort max-t` jointly compares a prespecified finite scalar endpoint family between two
independent patient groups. One common whole-patient label permutation is applied to every endpoint
per replicate; adjusted p-values use the replicate maximum absolute studentized statistic.

## Frozen behavior

- Strict CSV columns are `patient_id,group,endpoint,value`, with one finite value for every exact
  endpoint and patient, at least two patients per group, at least one endpoint, and no duplicate or
  incomplete patient-endpoint rows.
- Report each group-A-minus-group-B mean effect and Welch-style statistic, inclusive-plus-one Max-T
  adjusted p-value, the prespecified alpha, and a conservative empirical max-T critical value.
- Permutations are deterministic, unblocked, whole-patient, complete, and common across endpoints.
  Zero/non-finite standard error in any observed or permuted endpoint fails the run.
- A two-endpoint hand fixture matches both observed effects/statistics. A slow reference matches
  adjusted p-values and critical value; repeated output is deterministic.

## Non-goals

No endpoint selection, missing-endpoint imputation, blocks, step-down Max-T, pointwise curve testing,
durable project, backend registry, or biological/causal claim.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed the hand oracle and independent
  slow Max-T adjustment/critical-value reference alongside prior cohort tests.
- `cargo +1.96.0 test --locked --all-features --test cohort_max_t_cli` passed the end-to-end CLI
  hand oracle.
- Warning-denied package Clippy passed after the shared Welch contrast became the canonical scalar
  and Max-T owner.
