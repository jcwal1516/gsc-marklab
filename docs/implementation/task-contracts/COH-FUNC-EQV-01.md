# Task contract — COH-FUNC-EQV-01 paired functional equivalence band

Status: complete

Date: 2026-08-25

Parent requirements: EQV-01, CMP-01B, COH-01, INF-01D, WS-34.

`marklab cohort functional-equivalence` consumes one complete paired difference curve per patient
on an identical increasing physical axis and one identical prespecified positive margin curve. It
resamples whole patient curves, calculates each bootstrap mean curve, and uses the nearest-rank
`1-alpha` quantile of maximum absolute deviation from the observed mean for a simultaneous band.
Equivalence requires the entire band to lie strictly inside `[-margin,+margin]`; every failing scale,
the critical deviation, rationale, seed, and complete replicate count are retained. This is an
experimental percentile-bootstrap band, not pointwise equivalence or calibrated real-data evidence.
