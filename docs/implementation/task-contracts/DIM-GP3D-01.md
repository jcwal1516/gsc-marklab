# Task contract — DIM-GP3D-01 bounded anisotropic three-dimensional GP

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §95.4, BAY-04, BAY-FIELD-A, DIM-01, WS-42, WS-80.

`marklab bayes anisotropic-gp-3d` fits 8–64 finite scalar observations at unique physical
three-dimensional micrometre coordinates and predicts at 1–512 explicit coordinates. It uses the
existing locked Python 3.12 / PyMC 6.3.0 Apache-2.0 backend and exact dense NUTS inference.

Version one owns an axis-aligned anisotropic Matérn-3/2 kernel. The three positive inferred length
scales act on x/y/z coordinate differences before Euclidean norm; observations must vary on every
axis. Mean, amplitude, each axis length scale, and noise receive explicit proper priors. Jitter,
sampling, timeout, backend/lock/worker digests, and a conservative cubic conditioning-work bound are
part of the request identity.

The result retains all posterior parameter summaries, the diagonal metric evaluated at posterior
mean length scales, exact conditional prediction summaries, posterior-predictive field summaries,
and the established R-hat/ESS/MCSE/E-BFMI/divergence/tree-depth gates. It is experimental
axis-aligned interpolation only: it does not infer rotations, a tissue window, deformation,
nonstationarity, biological effects, or real-data validity.

The independent Rust kernel oracle proves covariance `a²` at zero distance and
`a²(1+sqrt(3))exp(-sqrt(3))` at one axis-scaled unit. The real worker fixture fits 12 observations,
returns a complete diagnostic state, and produces two finite positive-uncertainty predictions.
