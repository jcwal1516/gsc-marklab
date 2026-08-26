# Task contract — REG-SINKHORN-01 balanced log-domain Sinkhorn transport

Status: complete

Date: 2026-08-25

Parent requirements: FR-03, REG-01C.

## User outcome

`marklab bayes sinkhorn-ot` computes a bounded balanced entropic transport plan with explicit dual,
marginal, entropy, objective, and convergence artifacts.

## Frozen behavior

- consume 1–2,000 unique source/target IDs, finite nonnegative masses with equal positive totals, and
  one complete finite nonnegative row-major cost matrix; bound matrix × iteration work;
- update dual potentials in the log domain, center periodically, stop on maximum absolute marginal
  residual, and report convergence without inventing success at the iteration limit;
- remove zero-mass rows/columns from dual updates and restore exact zero plan entries/nullable duals;
- report the complete plan, marginals/residual, transport cost, entropy, and the convention
  `cost + epsilon * sum(P*(log(P)-1))`. A plan is descriptive alignment, not correspondence.

## Validation and claims

For equal two-point masses and cost `[[0,1],[1,0]]` at epsilon one, diagonal mass is
`0.5/(1+exp(-1))` and off-diagonal mass is its complement to `0.5`.
