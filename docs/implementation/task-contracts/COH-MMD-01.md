# Task contract — COH-MMD-01 patient-level maximum mean discrepancy

Status: complete

Date: 2026-08-24

Parent requirements: FND-06, COH-01, CMP-01C, INF-01A, WS-34.

## User outcome

`marklab cohort mmd` compares one prespecified complete finite fingerprint per patient between two
independent groups using one frozen kernel matrix and whole-patient label permutations.

## Frozen behavior

- Strict CSV columns are `patient_id,group,feature,value`; every patient has the identical exact
  ordered feature set, at least two patients occur in each group, and all values are finite.
- Version one supports exact linear and RBF kernels. Linear forbids bandwidth; RBF requires a finite
  positive prespecified bandwidth. The default estimator is unbiased U-statistic MMD-squared;
  biased V-statistic mode is separately named.
- The kernel matrix is built once. Each deterministic whole-patient label permutation reuses it and
  yields an inclusive-plus-one one-sided-high p-value. Kernel size and permutation work are bounded.
- A two-dimensional linear-kernel hand fixture has exact unbiased MMD-squared 11. A slow reference
  matches production linear/RBF values and permutation p-values.

## Non-goals

No data-dependent bandwidth selection, missing-feature imputation, blocks, kernel learning,
spatial curve kernel, durable project, external backend, or biological/causal claim.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed the exact linear hand oracle and
  independent slow linear/RBF statistic and p-value reference.
- `cargo +1.96.0 test --locked --all-features --test cohort_mmd_cli` passed the end-to-end linear
  U-statistic fixture; warning-denied cohort Clippy passed.
