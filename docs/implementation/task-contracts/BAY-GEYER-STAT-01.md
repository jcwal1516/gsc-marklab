# Task contract — BAY-GEYER-STAT-01 saturated Geyer statistic

Status: complete

Date: 2026-08-25

Parent requirements: BAY-PP, BAY-PP-B, WS-43, BAY-STRAUSS-STAT-01.

## User outcome

`marklab bayes geyer-saturation-statistic` computes the exact pointwise-saturated fixed-radius Geyer
interaction statistic under an explicit density convention.

## Frozen behavior

- reuse IC-0071 exact-ID finite 2-D micrometre points, positive finite radius, nonnegative integer
  saturation, bounded exact unordered-pair visits, and fresh output;
- count every unordered pair once under Euclidean `distance<=R`, increment both endpoints' neighbor
  counts, then return `sum_i min(s,neighbors_i)`; retain every point's raw/saturated count;
- saturation zero yields zero; detect count/work/non-finite overflow. Declare the pointwise-sum
  convention explicitly because other Geyer density definitions can differ;
- do not implement attraction by allowing Strauss gamma above one, and do not call this a fitted or
  proven-stable Geyer process.

## Validation and claims

For `(0,0),(3,4),(10,0)`, radius five, and saturation one, raw counts are `[1,1,0]` and the statistic
is two. This validates fixed-pattern sufficient-statistic mechanics only—not normalizability,
inference, simulation, biology, causality, or clinical use.
