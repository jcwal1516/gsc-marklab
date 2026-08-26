# Task contract — CAUSAL-EXPOSURE-MAP-01 explicit spatial exposure mappings

Status: complete

Date: 2026-08-25

Parent requirements: Part XII §104.1 `ComputeExposureMapping`, CAU-01A, WS-82.

`marklab causal exposure-mapping` consumes unique binary-treated units, a prespecified unique
undirected graph with positive finite edge weights and physical distances, one explicit mapping
kind, and an exact directed edge-visit maximum. `binary_any_treated` returns zero/one;
`count_treated` returns treated-neighbour count; `weighted_fraction_treated` returns weighted treated
mass divided by total incident weight and marks isolates unavailable; `gaussian_distance_decay`
returns `sum exp(-0.5*(distance/bandwidth)^2)*Z_j` with positive micrometre bandwidth;
`multiscale_count_treated` returns cumulative treated-neighbour counts at strictly increasing
nonnegative micrometre radii; `continuous_field` returns a finite caller-declared value per unit.

Every result retains unit/treatment identity, scalar or vector shape, availability, mapping
parameters/provenance, and exact directed visits. The hand graph has a central untreated unit joined
to one treated neighbour by weight two at distance one and one untreated neighbour by weight one at
distance three; its count is one, weighted fraction `2/3`, and unit-bandwidth Gaussian exposure
`exp(-1/2)`. This computes prespecified exposures only and makes no causal, assignment, outcome, or
effect claim.
