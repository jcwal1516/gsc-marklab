# Task contract — REG-PARTIAL-OT-01 constrained entropic partial transport

Status: complete

Date: 2026-08-25

Parent requirements: FR-03, REG-01C, BACK-01.

## User outcome

`marklab bayes partial-ot` solves a fixed-mass entropic partial transport problem and reports unmatched
source/target mass with independently replayed feasibility.

## Frozen behavior

- consume 1–64 source/target supports, nonnegative capacities, complete finite nonnegative costs,
  positive transported mass no larger than either total, epsilon, and bounded worker resources;
- use pinned SciPy 1.18.1 SLSQP over nonnegative plan variables with row/column capacity inequalities
  and exact total-mass equality; require successful optimization and `1e-8` feasibility;
- report the complete plan, marginals, unmatched capacities, transport cost, entropy, regularized
  objective, optimizer diagnostics, and maximum violation; Rust independently recomputes all of them;
- remain descriptive transport. Unmatched mass is not automatically novel biology and plan mass is not
  cell identity.

## Validation and claims

For one cell with source capacity two, target capacity three, transported mass `1.5`, cost four, and
epsilon `0.5`, the forced plan is `1.5`, unmatched mass is `0.5/1.5`, and transport cost is six.
