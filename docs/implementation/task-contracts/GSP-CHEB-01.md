# Task contract — GSP-CHEB-01 adaptive Chebyshev heat application

Status: complete

Date: 2026-08-25

Parent requirements: SCALE-01, GSP-01, FR-01A, FR-01B, WS-62, GSP-HEAT-01.

`marklab graph chebyshev-heat` reuses the canonical Laplacian and exact spectrum, scales the
operator from `[0,lambda_max]` to `[-1,1]`, computes deterministic DCT Chebyshev coefficients for
`exp(-t lambda)`, and selects the first order within 1–256 whose truncated reference tail and
4,097-point grid error both meet caller tolerance. The generic `chebyshev_apply` recurrence is
immediately consumed, and the approximate signal is differentially checked against exact spectral
heat application before publication. Output calls the error evidence a dense-grid plus truncated
reference-tail estimate, not a continuous certificate. The path lambda-one signal matches
`exp(-1)[1,0,-1]` within `1e-8`.
