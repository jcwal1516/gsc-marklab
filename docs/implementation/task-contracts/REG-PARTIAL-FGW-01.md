# Task contract — REG-PARTIAL-FGW-01 entropic partial fused Gromov-Wasserstein

Status: complete for the fixed-mass branch; KL-unbalanced branch backend-blocked

Date: 2026-08-25

Parent requirements: FR-03A, FR-03B, REG-01C, BACK-01.

## User outcome

`marklab bayes partial-fused-gromov-wasserstein` returns an ensemble of bounded, fixed-mass
descriptive FGW plans with explicit unmatched mass and sensitivity to alpha, mass, epsilon, and
initialization.

## Frozen behavior

- consume 1–32 strictly positive probability-mass supports, finite feature vectors, finite
  nonnegative symmetric zero-diagonal structure matrices, transported mass strictly between zero
  and one, explicit cost scales, alpha, epsilon, tolerance, iteration limit, and timeout;
- linearize the squared FGW structural term at each outer iteration and solve every constrained
  entropic fixed-mass subproblem with pinned POT 0.9.7.post1 log-domain partial Wasserstein;
- do not call POT 0.9.7's convenience partial-FGW wrapper: its feature-gradient implementation
  collapses the feature-cost matrix to a scalar, reproduced by the independent-mass reversed-feature
  regression;
- preserve three base initialization plans and seven one-axis-at-a-time alpha/mass/epsilon fits;
  independently replay all capacities, transported/unmatched mass, objectives, convergence states,
  and best-base-plan selection in Rust;
- return an ensemble descriptive alignment claim only. The KL-relaxed unbalanced-FGW branch remains
  blocked because the pinned established backend exposes partial FGW but no FGW solver with KL-relaxed
  source and target marginals; unbalanced co-optimal transport is a different estimand.

## Validation and claims

A one-cell half-mass fixture has plan mass `0.5` and unmatched source/target mass `0.5`. A two-point
reversed-feature isometry moves the independent-mass initialization to more than `0.45` cross mass,
which guards the corrected matrix-valued feature gradient.
