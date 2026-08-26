# Task contract — BAY-CAR-01 proper and intrinsic CAR density

Status: complete

Date: 2026-08-24

Parent requirements: BAY-05, BAY-GMRF-A, WS-42.

## User outcome

`marklab bayes car-density` consumes IC-0047 preserved symmetric zero-diagonal weights and an exact
region field, constructs either proper or intrinsic CAR precision, and evaluates its normalized
full-space or constrained-subspace GMRF log density.

## Frozen behavior

- proper mode requires no islands, positive tau, finite rho, and positive-definite
  `Q=tau*(D-rho*W)` proven by Cholesky. Density includes full log determinant and dimension constant;
- intrinsic mode uses `Q=tau*(D-W)`, requires each non-island component field to sum to zero within
  explicit tolerance, records one rank deficiency/constraint per component, and computes the
  constrained determinant using `log(k)+log(det(any cofactor))` for each k-node component;
- version-one island policy is `reject` or `exclude`; excluded island values do not enter density
  and remain explicit. No iid island prior is silently supplied;
- fields bind every region exactly once. Version one is limited to 2–512 regions and 200,000 edges
  under dense diagnostic factorization bounds.

## Validation and claims

For a three-node chain, field `[0.2,-0.1,-0.1]`, and tau 1.5, proper rho 0.2 must yield log density
`-1.8779553444319261`; intrinsic sum-to-zero density must yield `-0.9506058139671261`. The result is
an experimental field-density diagnostic, not fitted posterior, disease map, biology, causality, or
clinical evidence.
