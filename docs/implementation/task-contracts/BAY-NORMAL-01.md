# Task contract — BAY-NORMAL-01 pinned PyMC normal-mean NUTS lifecycle

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BACK-01, WS-40.

## User outcome

`marklab bayes normal-mean` fits one finite scalar observation vector with a Normal prior on the
unknown mean, known positive observation standard deviation, and PyMC NUTS. Marklab owns the typed
model IR, exact backend/environment contract, finite JSON boundary, diagnostics normalization,
posterior predictive summary, and claim ceiling.

## Backend contract

- PyMC 6.3.0 from verified PyPI metadata, Apache-2.0, published 2026-08-12, pinned with all
  transitive dependencies in a repository-local `uv` lock using Python 3.12. No global package
  installation and no arbitrary process command.
- One static worker path and one exact request/result JSON version. The request binds backend/model
  version, prior/likelihood conventions, observations, chains, tune/draw counts, target acceptance,
  seed, and resource limits. Unknown fields/versions, stderr-only success, nonzero exit, malformed
  JSON, missing diagnostics, and non-finite values are errors.

## Scientific behavior and oracle

- Model IR: `mu ~ Normal(prior_mean, prior_sd)` and `y_i ~ Normal(mu, known_sigma)` with one scalar
  parameter, explicit support/interpretation, likelihood observation unit, generated posterior
  predictive mean, backend capability `nuts`, and experimental maturity until broader calibration.
- Run prior predictive finite checks, NUTS sampling, rank-normalized R-hat, bulk/tail ESS,
  divergences, maximum tree-depth hits, posterior finiteness, and posterior predictive observed-mean
  discrepancy. Nonconverged diagnostics return a typed nonconverged result rather than success.
- For prior `Normal(0,1)`, known sigma 1, and observations `[1,2,3,4]`, the analytic posterior is
  `Normal(2, sqrt(0.2))`. The focused fit must recover mean and SD within prespecified Monte Carlo
  tolerance and report zero divergences with complete chains/draws.

## Non-goals

No generalized backend registry, arbitrary model language, GPU requirement, variational/SMC/INLA,
hierarchical/spatial model, stable clinical claim, durable project integration, remote execution,
or native HMC/NUTS port in this workflow.

## Delivered evidence

- `marklab bayes normal-mean` owns the strict one-column `observation` CSV boundary, typed
  `marklab.bayesian_model_ir` version-one Normal/Normal model, bounded static process call, strict
  request/result schemas, finite normalization, and failure-atomic JSON publication.
- Backend: PyMC 6.3.0 (Apache-2.0) on Python 3.12, locked with all transitive packages in
  `workers/python/uv.lock`. Lock SHA-256:
  `9dcf32020dabef2ecb6b4b657179985d3177d2b773e7569d98051264ef129c5c`. The PyPI wheel hash retained
  by the lock is `b8751545ddf66e16c9b67317b42b3babdab3a9328f06f11d76b3b4ca3f5a672a`.
- Exact local environment command: `UV_PROJECT_ENVIRONMENT=/Users/user/Bench/gsc-marklab/target/pymc-venv /Users/user/.local/bin/uv sync --locked --python /usr/local/bin/python3.12 --no-python-downloads`, run from `workers/python`.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 6 unit/contract tests. `cargo
  +1.96.0 test --locked --test bayes_normal_mean_cli -- --nocapture` passed 2 backend integration
  tests: the analytic posterior `Normal(2, sqrt(0.2))`, zero-divergence complete diagnostics,
  byte-identical seeded reruns, and forced ESS-policy nonconvergence.
