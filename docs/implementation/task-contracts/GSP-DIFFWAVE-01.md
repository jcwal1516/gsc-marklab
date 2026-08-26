# Task contract — GSP-DIFFWAVE-01 diffusion wavelets

Status: complete

Date: 2026-08-25

Owns `BuildDiffusionWaveletTree` and `DiffusionWaveletTransform` through IC-0150. The consumed
`marklab graph diffusion-wavelet` path reuses the canonical exact graph eigensystem, retains modes
of dyadic lazy-operator powers above the declared tolerance, and emits scaling/detail bases and exact
signal reconstruction. Evidence is the three-node path rank sequence `2,1` and lambda-one detail.
Sparse localized bases, physical-scale calibration, and pathology validation are non-goals.
