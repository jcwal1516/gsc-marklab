# Task contract — NUM-STABLE-01 canonical stable numerical primitives

Status: complete

Date: 2026-08-25

Parent requirements: Part XIII §124 `StableLogSumExp`, `StableLogMeanExp`, `StableWeightedMean`, and
`StableCovariance`.

`marklab numerics stable-primitives` consumes bounded nonempty finite log values; equally sized
finite scalar values and nonnegative finite weights with positive sum; and a bounded rectangular
finite observation matrix with at least two rows. Optional covariance weights must be nonnegative,
finite, correctly sized, and have effective denominator `1-sum(normalized_weight^2)>0`.

Log-sum-exp subtracts the maximum and uses compensated summation; log-mean-exp subtracts log count.
Weighted mean normalizes by a compensated weight sum and uses Neumaier compensated products.
Covariance uses a two-pass compensated mean/cross-product calculation, divides unweighted results by
`n-1` or weighted results by `1-sum(w^2)`, and writes exact symmetric entries from one accumulated
triangle. Every output must be finite; input element/work maxima are validated before allocation.

The oracle checks `[1000,999]` without overflow, weighted mean of `[1e16,1,-1e16]` under equal
weights equals `1/3`, and covariance of rows `[1,2],[2,4],[3,6]` equals `[[1,2],[2,4]]`. This is a
deterministic numerical primitive contract, not a fitted-model or performance claim.
