# Task contract — REG-UNBAL-OT-01 KL-unbalanced log-domain Sinkhorn

Status: complete

Date: 2026-08-25

Parent requirements: FR-03, REG-01C.

## User outcome

`marklab bayes unbalanced-sinkhorn` transports unequal source/target mass with explicit KL marginal
penalties and a fully decomposed objective.

## Frozen behavior

- reuse exact transport identities/cost/resource rules without requiring equal total mass; require
  positive epsilon, source/target KL penalties, tolerance, and bounded iterations;
- perform scaling updates in the log domain with exponents `tau/(tau+epsilon)` and converge on maximum
  log-scaling change; retain nonconvergence explicitly;
- report plan, transported mass, relaxed marginals/deviations, transport/entropy/source-KL/target-KL
  terms, and their exact objective sum; preserve zero-mass supports as exact zero plan rows/columns.

## Validation and claims

For one zero-cost cell with source mass two, target mass eight, and epsilon/tau values one, the unique
fixed-point transported mass is `16^(1/3)` and both KL penalties are positive.
