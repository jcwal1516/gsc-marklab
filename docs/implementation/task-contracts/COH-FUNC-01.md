# Task contract — COH-FUNC-01 patient-level functional permutation

Status: complete

Date: 2026-08-24

Parent requirements: FND-06, COH-01, INF-01A, WS-31, WS-34.

## User outcome

`marklab cohort functional-permutation` compares one common-axis finite curve per patient between
two declared independent groups using a prespecified joint curve statistic and whole-patient label
permutations. Version one first admits the L2 statistic; pointwise p-values are not produced.

## Frozen behavior

- Strict CSV columns are `patient_id,group,axis,value`, with at least two patients per group, one
  strictly increasing identical finite axis per patient, no duplicate patient-axis rows, and at
  least two axis points.
- The observed curve is mean-A minus mean-B. L2 is its trapezoidal integral of squared difference
  over the physical axis. Labels shuffle only as whole patient curves within optional future design
  extensions; version one is unblocked independent groups.
- Output records the axis, group mean and difference curves, L2 statistic, inclusive-plus-one
  one-sided-high p-value, complete replicate counts, seed, and patient randomization unit.
- A hand fixture has constant difference 2.5 on axis `[0,1,2]` and exact L2 value 12.5. A slow
  reference matches production permutations; malformed axes/rows publish no output.

## Non-goals

No pointwise inference, ERL envelope, blocks, paired curves, interpolation, smoothing, Max-T,
durable project, backend registry, or biological/causal claim in this slice.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed seven unit tests and the patient,
  paired, and functional slow differential references.
- `cargo +1.96.0 test --locked --all-features --test cohort_functional_permutation_cli` passed
  the exact L2 hand-oracle workflow.
