# Task contract — BAY-LAPLACE-01 Poisson log-rate Laplace approximation

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, WS-40.

## User outcome

`marklab bayes poisson-log-rate-laplace` computes a mode/Hessian Gaussian approximation for one
Poisson log-rate model through pinned SciPy/PyTensor mechanics and returns an explicitly
approximate-only artifact.

## Frozen behavior

- input has 1–100,000 nonnegative integer counts and matching finite positive exposures;
- scalar unconstrained `theta=log(rate)` has caller-declared Normal prior; counts follow
  `Poisson(exposure*exp(theta))`;
- pinned SciPy robust optimization uses exact PyTensor log-joint gradient from an explicit finite
  initial theta, bounded iterations, and positive finite gradient tolerance. Convergence requires
  optimizer success and absolute gradient within tolerance;
- negative Hessian at the mode is obtained from the exact differentiated log joint, must be finite
  and positive, and yields covariance/inverse conditioning diagnostics. Log-rate approximation is
  Normal and rate summaries use the declared lognormal transformation/delta semantics;
- strict environment/worker/request/data identities, bounded runtime/output, mode/gradient/Hessian
  checks, and posterior/PPC finiteness are retained. Output is always `approximate_only` when valid,
  otherwise `nonconverged`; never `complete` exact posterior.

## Validation and claims

For counts `[0,1,2,3,4]`, unit exposures, and prior `Normal(0,1)`, the mode is
`0.6282607821567117`, negative Hessian `10.371739217843288`, Laplace variance
`0.09641584492209603`, and transformed approximate rate mean `1.966919679609662`. This validates
approximation mechanics only—not exact inference, model selection, science, biology, causality, or
clinical evidence.
