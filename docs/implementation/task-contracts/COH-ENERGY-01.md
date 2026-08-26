# Task contract — COH-ENERGY-01 patient-level energy distance

Status: complete

Date: 2026-08-24

Parent requirements: FND-06, COH-01, CMP-01D, INF-01A, WS-34.

## User outcome

`marklab cohort energy` compares one complete finite fingerprint per patient between two independent
groups using a frozen exact distance matrix and whole-patient label permutations.

## Frozen behavior

- Reuse the strict `patient_id,group,feature,value` complete-fingerprint boundary.
- Version one supports exact Euclidean distance, a negative-type metric. Report
  `2 mean(D_ab) - mean(D_aa') - mean(D_bb')`, where both within-group means include ordered
  diagonals as the standard energy V-statistic.
- Build the distance matrix once, reuse it for deterministic whole-patient permutations, and report
  an inclusive-plus-one one-sided-high p-value under explicit matrix/work limits.
- A scalar hand fixture with A=`[3,5]` and B=`[0,1]` has exact energy distance 5.5. A slow reference
  matches the statistic and p-value.

## Non-goals

No learned or non-negative-type metric, missing features, blocks, weighted patients, distance
approximation, durable project, external backend, or biological/causal claim.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed the exact scalar hand oracle and
  independent slow Euclidean statistic/p-value reference.
- `cargo +1.96.0 test --locked --all-features --test cohort_energy_cli` passed the end-to-end
  fixture; warning-denied cohort Clippy passed.
