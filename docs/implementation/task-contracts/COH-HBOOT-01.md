# Task contract — COH-HBOOT-01 patient-first hierarchical scalar bootstrap

Status: complete

Date: 2026-08-24

Parent requirements: FND-06, COH-01, INF-01D, WS-34.

## User outcome

`marklab cohort hierarchical-bootstrap` estimates uncertainty for one prespecified scalar mean from
patient→specimen data by resampling patients first and specimens only inside each sampled patient
occurrence.

## Frozen behavior

- Strict CSV columns are `patient_id,specimen_id,endpoint`, with unique non-empty specimen IDs,
  exact non-empty patient parents, finite endpoints, at least two patients, and at least one
  specimen per patient.
- Observed statistic is the stable mean across supplied specimen endpoint rows. Each deterministic
  replicate samples the original patient count with replacement; for every sampled patient
  occurrence it samples that patient's original specimen count with replacement, preserving
  occurrence multiplicity. No cell/specimen-only fallback is allowed.
- Report observed mean, requested/attempted/completed counts, patient/specimen counts, seed, alpha,
  and an explicitly named percentile interval using deterministic empirical order statistics.
  Every replicate completes or the workflow fails.
- A hand fixture matches the observed mean and an independent slow reference matches all replicate
  values and interval bounds; repeated CLI output is byte-identical.

## Non-goals

No cells/patches, deeper hierarchy, BCa/jackknife, missing-child policy, paired/repeated design,
weighted statistic, durable project, or biological/clinical claim in version one.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed the hand mean/count oracle and an
  independent slow patient-first replicate/interval reference.
- `cargo +1.96.0 test --locked --all-features --test cohort_hierarchical_bootstrap_cli` passed
  patient/specimen design reporting and byte determinism; warning-denied cohort Clippy passed.
