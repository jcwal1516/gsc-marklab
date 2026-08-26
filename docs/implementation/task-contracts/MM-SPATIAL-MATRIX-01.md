# MM-SPATIAL-MATRIX-01 — Graph-spatial Bayesian matrix factors

Own `SpatialBayesianMatrixFactorization` through IC-0170 and `marklab multimodal
spatial-matrix-factor`. The missing-command red preceded implementation; the first optimizer run hit
its iteration cap, then alternating exact conditional initialization made the unchanged smooth
path-graph oracle pass four masked values below RMSE 0.35. Pinned SciPy 1.18.1 results remain bounded
synthetic `approximate_only`; graph adequacy, sparse scale, and exact calibration are unverified.
