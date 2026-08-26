# Task contract — MM-PCCA-01 paired Gaussian pCCA

Status: complete

Date: 2026-08-25

Owns the paired patient/Gaussian specialization of `ValidateMultimodalDesign` and
`FitProbabilisticCCA`. `marklab multimodal pcca` uses pinned NumPy 2.4.6/SciPy 1.18.1 for
training-only standardization and EM with diagonal per-view noise. The synthetic shared-direction
oracle passes canonical correlation and held-out cross-view prediction gates. Other likelihoods,
entity levels, incomplete rows, and real claims remain outside this specialization.
