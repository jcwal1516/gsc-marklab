# Task contract — BAY-GMRF-01 general constrained GMRF density

Status: complete

Date: 2026-08-24

Parent requirements: BAY-05, BAY-GMRF-A, WS-42.

## User outcome

`marklab bayes gmrf-density` consumes an exact named field, a declared symmetric precision matrix,
and zero or more named homogeneous linear constraints, then evaluates the normalized Gaussian density
on the admissible constrained subspace.

## Frozen behavior

- region IDs are exact, unique, and lexically canonicalized; fields bind every region exactly once;
- precision input is a unique sparse triplet representation with implicit zeros, all diagonal entries
  explicit, and exact finite symmetry. No silent symmetrization, jitter, or eigenvalue repair occurs;
- constraints are named nonempty finite coefficient rows, independently ranked under an explicit
  tolerance. The field must satisfy every constraint within that tolerance;
- deterministic reorthogonalized modified Gram–Schmidt constructs an orthonormal null-space basis.
  The projected precision must be positive definite by Cholesky. Density includes the projected log
  determinant and the constant for dimension `n-rank(C)`;
- version one is limited to 2–256 regions, at most 65,536 stored precision entries, 0–255
  constraints, and positive finite tolerance.

## Validation and claims

For `Q=diag(2,3,4)`, constraint `x_a+x_b=0`, and field `[0.5,-0.5,0.25]`, the projected
precision is `diag(2.5,4)`, the quadratic is `1.5`, and the log density must be
`-1.4365845199123224`. The result is an experimental field-density diagnostic, not fitted
posterior, spatial adjacency proof, biology, causality, or clinical evidence.
