# MM-MULTIRES-01 — Multiresolution spatial factors

Own `FitMultiresolutionSpatialFactors` through IC-0173. The missing-command red preceded
implementation. Pinned JAX/SciPy jointly fits declared coarse/fine physical bases; both contribute
positive variance, fractions sum to one, and four masks pass RMSE 0.25. Results remain bounded
synthetic `approximate_only`; basis adequacy is not inferred.
