# Task contract — BAY-PCCA-01 Bayesian pCCA

Status: complete

Date: 2026-08-25

Owns `FitBayesianPCCA` through pinned CCA-Zoo 3.0.0, NumPyro 0.21.0, and JAX 0.11.1. The static
adapter retains the CCA-Zoo generative model while preserving NUTS divergence diagnostics, aligns
all one-factor draws by first-view loading sign, and evaluates held-out cross-view prediction. The
zero-divergence synthetic oracle passes; multi-factor rotation alignment and real calibration remain
open.
