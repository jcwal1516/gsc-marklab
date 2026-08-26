# Task contract — LONG-KALMAN-01 linear-Gaussian filtering and smoothing

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §99.1, DIM-01, WS-81.

## Accepted input

`marklab longitudinal kalman-smooth --input <json> --out <json>` accepts a bounded sequence of
finite observations. Each observation component is a number or `null`; `null` means that component
is unobserved at that time. The document declares one initial state mean and covariance plus exactly
one transition matrix, process covariance, observation matrix, and observation covariance per time
step. Matrix dimensions must agree. Initial, process, and observation covariances must be finite,
symmetric, and positive semidefinite. Initial and observation covariances must be positive definite;
the implementation rejects numerically singular innovation and predicted smoother covariances
rather than silently regularizing them.

The input also declares positive `maximum_time_steps`, `maximum_state_dimension`,
`maximum_observation_dimension`, and `maximum_matrix_operations`. The implementation validates the
entire execution plan against these limits before filtering.

## Observable output

The result contains every predicted, filtered, and RTS-smoothed state mean/covariance, the summed
observed-component Gaussian innovation log likelihood, per-step observed-component counts, the
declared matrix-operation bound and exact observed update count, and fixed algorithm/format labels.
Missing components are removed from the observation equation for that time step; an all-missing row
performs prediction without an update or likelihood contribution.

## Numerical and claim contract

Innovation and smoother systems are solved by Cholesky factorization without forming a matrix
inverse. Filtered covariance uses the Joseph update and all returned values must remain finite and
symmetric. This is an exact finite-dimensional linear-Gaussian calculation within floating-point
roundoff, not a nonlinear, non-Gaussian, spatial-field, particle, or biological validation claim.

## Initial oracle

For scalar `F=H=1`, zero process variance, `R=P0=1`, `m0=0`, and observations one and two, the CLI is
checked against an independent hand calculation, with a separate package test covering multivariate
and missing-component paths.
