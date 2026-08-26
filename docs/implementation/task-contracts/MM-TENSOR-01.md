# MM-TENSOR-01 — Bayesian CP and Tucker factors

Own `BayesianCPFactorization` and `BayesianTuckerFactorization` through IC-0171 and `marklab
multimodal tensor-factor`. Both missing-command reds preceded implementation. Pinned JAX
0.11.1/SciPy 1.18.1 CP and Tucker adapters independently recover three masked values in an exact
3x3x3 rank-one tensor below RMSE 0.25 with explicit alignment. Results remain three-mode Gaussian,
synthetic, and `approximate_only`; HMC agreement, rank/interval calibration, and scale are unverified.
