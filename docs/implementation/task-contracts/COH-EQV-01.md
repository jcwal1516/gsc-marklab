# Task contract — COH-EQV-01 patient-level TOST equivalence

Status: complete

Date: 2026-08-24

Parent requirements: EQV-01, COH-01, INF-01D, WS-34.

## User outcome

`marklab cohort equivalence` tests whether one prespecified finite patient-level effect lies within
prespecified scientifically justified lower and upper margins using two one-sided Student-t tests.

## Frozen behavior

- Strict CSV columns are `patient_id,effect`, with one finite effect per unique non-empty patient
  and at least two patients.
- CLI requires finite `lower_margin < upper_margin`, finite `0<alpha<0.5`, and a non-empty exact
  margin-rationale reference. Effects use the declared sign convention without inference from files.
- Report stable mean effect, sample standard error, `n-1` degrees of freedom, both one-sided
  statistics/p-values, the matching two-sided `1-2*alpha` confidence interval, and equivalence only
  when both p-values are below alpha and the interval lies strictly inside the margins.
- Zero/non-finite standard error is an error. A fixed hand fixture is differentially checked against
  R 4.5.2 Student-t values; failure publishes no output.

## Non-goals

No unpaired group model, Welch-Satterthwaite degrees of freedom, functional equivalence, bootstrap,
post-hoc margins, power claim, durable project, or biological/clinical claim.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed the R 4.5.2 Student-t static
  oracle for standard error, statistics, p-values, and matching confidence interval.
- `cargo +1.96.0 test --locked --all-features --test cohort_equivalence_cli` passed the complete
  TOST workflow; warning-denied cohort Clippy passed.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed exact R 4.5.2 standard error,
  statistics, p-values, and matching confidence bounds.
- `cargo +1.96.0 test --locked --all-features --test cohort_equivalence_cli` passed the complete
  TOST workflow and strict-margin decision.
