# Task contract — BAY-PREDPROC-01 low-rank predictive-process diagnostic

Status: complete

Date: 2026-08-24

Parent requirements: BAY-04, BAY-FIELD-A, SCALE-01, WS-42.

## User outcome

`marklab bayes predictive-process` constructs the deterministic low-rank Matérn-3/2 field
representation for explicit one-dimensional micrometre coordinates and knots, reports residual
variance/trace loss, and optionally declares the standard diagonal correction.

## Frozen behavior

- `Kmm = K(knots,knots) + jitter I`, `Knm = K(coordinates,knots)`, and
  `low_rank = Knm * inverse(Kmm) * Knm^T`;
- residual variance is `max(K(x_i,x_i)-low_rank_ii,0)` after rejecting materially negative
  numerical residuals. Diagonal correction adds this residual only to the representation diagonal;
- version one uses the IC-0042 Matérn-3/2 micrometre range convention with caller-supplied positive
  amplitude/length/jitter, 2–128 unique knots, 2–2,000 unique coordinates, and checked
  `O(m^3+n*m^2)` work/memory limits.

The API owns Kmm Cholesky, Knm, and residual diagonal; the CLI publishes bounded per-coordinate
residuals and aggregate trace/maximum/mean fractions rather than a dense `n*n` matrix.

## Validation and claims

A hand fixture with coordinates equal to both endpoint knots must have zero endpoint residual and a
positive interior residual; diagonal correction must make every represented marginal variance
equal the exact kernel diagonal. The result is an approximation diagnostic, not exact GP inference,
calibrated uncertainty, fitted field, tissue validation, biology, or clinical evidence.

## Delivered evidence

- `marklab_bayes::low_rank_predictive_process` owns the exact Matérn Kmm Cholesky, Knm rows,
  low-rank diagonal, residual diagonal, negative-residual tolerance, and checked storage/work caps.
  `marklab bayes predictive-process` is its immediate caller and publishes per-coordinate plus
  trace-loss diagnostics without materializing `n*n` output.
- The hand fixture passes both unit and CLI integration tests: endpoint-knot residuals are at most
  `1e-9`, the interior residual is positive, and diagonal correction restores all exact unit
  marginal variances within `1e-10`.
