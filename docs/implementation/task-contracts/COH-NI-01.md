# Task contract — COH-NI-01 patient-level noninferiority

Status: complete

Date: 2026-08-24

Parent requirements: EQV-01, COH-01, INF-01D, WS-34.

## User outcome

`marklab cohort noninferiority` tests one prespecified finite patient-level effect against one
prespecified scientifically justified noninferiority margin with an explicit favorable direction.

## Frozen behavior

- Reuse strict `patient_id,effect` input, with at least two unique patients and finite effects.
- Require positive finite margin, finite `0<alpha<0.5`, non-empty margin-rationale reference, and
  `higher-is-better` or `lower-is-better` direction.
- Higher-is-better tests boundary `-margin` and requires the one-sided lower confidence bound above
  it. Lower-is-better mirrors at `+margin` and requires the upper bound below it. Report estimate,
  standard error, degrees of freedom, boundary, favorable-direction statistic, upper-tail p-value,
  one-sided bound, and decision.
- A symmetric fixture is checked against R 4.5.2 Student-t values. Noninferiority is never labeled
  equivalence.

## Non-goals

No superiority, equivalence, unpaired group model, functional endpoint, bootstrap, post-hoc margin,
power claim, durable project, or biological/clinical claim.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed both directional unit cases and
  the R 4.5.2 Student-t reference for SE, statistic, p-value, and one-sided bound.
- `cargo +1.96.0 test --locked --all-features --test cohort_noninferiority_cli` passed the
  higher-is-better workflow; warning-denied cohort Clippy passed.
