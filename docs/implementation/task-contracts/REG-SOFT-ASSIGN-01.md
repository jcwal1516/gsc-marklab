# Task contract — REG-SOFT-ASSIGN-01 entropic soft assignment with dustbins

Status: complete

Date: 2026-08-25

Parent requirements: REG-01C, FR-03.

## User outcome

`marklab bayes entropic-soft-assignment` computes a bounded soft compatibility plan with explicit
unmatched source and target states and epsilon sensitivity.

## Frozen behavior

- consume unique source/target IDs, positive finite total masses, one complete finite nonnegative
  cost matrix, positive epsilon, finite nonnegative dustbin cost, tolerance, and bounded work;
- augment source mass by the target total and target mass by the source total so the balanced
  log-domain Sinkhorn primitive preserves all real mass while permitting unmatched assignments;
- assign the declared dustbin cost to every edge incident to a dustbin and report real-plan,
  source-to-dustbin, target-from-dustbin, and dustbin-to-dustbin mass separately;
- evaluate half, base, and double epsilon under identical controls and report convergence, real and
  unmatched totals, and regularized cost for every fit;
- interpret the result only as probabilistic compatibility, never physical cell identity.

## Validation and claims

For one unit-mass source and target, real cost ten, dustbin cost zero, and epsilon `0.1`, real-plan
mass is below `0.01` while each unmatched mass exceeds `0.99`.
