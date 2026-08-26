# Task contract — LONG-NONLINEAR-01 scalar EKF/UKF

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §99.2, DIM-01, WS-81.

`marklab longitudinal nonlinear-filter` consumes bounded scalar observations with explicit missing
values and one analytic quadratic transition and observation function per step. The method is
exactly `ekf` or `ukf`; EKF uses analytic derivatives, while UKF uses three scalar sigma points with
caller-declared positive finite alpha, beta, and kappa settings whose scaling denominator is
positive. Process variance may be zero; initial and observation variances are positive. Every
intermediate mean, variance, innovation, gain, derivative/sigma spread, and likelihood term must be
finite, and negative variance beyond roundoff is an error.

The result retains predicted and filtered scalar moments plus per-step method diagnostics,
observed-update accounting, Gaussian moment-matched innovation log likelihood, and an explicit
approximation ceiling. A linear identity model must reduce to the exact scalar Kalman hand oracle
for both methods. This workflow does not claim nonlinear smoothing, non-Gaussian calibration,
multivariate generality, or biological validity.
