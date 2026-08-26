# Task contract — BAY-NNGP-01 one-dimensional nearest-neighbor GP plan and density

Status: complete

Date: 2026-08-24

Parent requirements: BAY-04, BAY-FIELD-A, SCALE-01, WS-42.

## User outcome

`marklab bayes nngp-density` constructs a deterministic one-dimensional physical-order NNGP plan
for a supplied finite field and evaluates its sparse directed conditional log density. On bounded
small inputs it can also report the exact full-GP log-density difference.

## Frozen behavior

- order unique coordinates by increasing micrometre value, breaking no ties because duplicate
  coordinates are invalid;
- for row `i`, use the nearest `min(m,i)` predecessors. Under ascending 1-D physical order these are
  exactly the immediately preceding rows;
- compute `B_i = C_iN C_NN^-1` and `F_i = Kii - B_i C_Ni` with Matérn-3/2 covariance and explicit
  jitter in predecessor solves. Require finite `F_i` above numerical tolerance;
- `NNGPLogDensity` sums Normal conditional log densities of centered field residuals and never drops
  a row.

Version one accepts 2–10,000 rows, 1–64 neighbors, positive amplitude/length/jitter/tolerance, and
checked `O(n*m^3)` work. Full dense reference is optional and limited to 128 rows.

## Validation and claims

For four coordinates and `m=3`, NNGP conditions on every predecessor and must match the independently
computed full-GP log density within `1e-9`. The result is an experimental covariance-density
approximation diagnostic, not fitted inference, calibrated prediction, tissue validation, biology,
or clinical evidence.

## Delivered evidence

- `build_nngp` owns ascending physical ordering, last-`m` nearest predecessor sets, per-row Matérn
  covariance Cholesky, B coefficients, positive F variances, and the 500-million-unit work cap.
  `nngp_log_density` consumes every row; `marklab bayes nngp-density` is the immediate user caller.
- Package and CLI hand oracles pass: neighbor counts are `[0,1,2,3]`, every F is positive, and with
  all predecessors the NNGP log density matches an independently factored full covariance within
  `1e-9`.
