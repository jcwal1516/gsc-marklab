# Normal-mean prior sensitivity

A Bayesian conclusion depends partly on the prior. This command answers a practical
question: if you had picked a different reasonable prior, would your conclusion change?

You give a set of measurements, a list of candidate Normal priors for their mean, and a
decision rule such as "the mean is above 1 with at least 95% probability". For each prior,
Marklab computes the exact posterior, whether the decision holds, how far the posterior
mean moves compared with your base prior, and how well that prior predicts each
measurement when it is held out. The measurements are assumed to be independent and
Normal with a known standard deviation.

It runs entirely in Rust; Python is not needed.

## Run

Create `observations.csv` with the header `observation` and one number per row:

```csv
observation
1
2
3
4
```

Create `priors.csv` with this header. Names must be unique, non-empty and have no
leading or trailing spaces; each prior standard deviation must be positive.

```csv
prior_name,prior_mean,prior_sd
base,0,1
skeptical,-2,0.5
wide,0,10
```

Run it with a new output path:

```sh
marklab bayes normal-mean-prior-sensitivity \
  --input observations.csv --priors priors.csv --base-prior base \
  --known-sigma 1 --decision-threshold 1 \
  --decision-probability-threshold 0.95 --material-mean-shift 0.5 \
  --timeout-seconds 60 --out sensitivity.json
```

In this example the base prior gives a posterior mean of 2 with SD about 0.447214,
`P(mu > 1)` about 0.987326, and a leave-one-out score (LOO ELPD) of about -7.872041. The
skeptical prior changes the decision and moves the posterior mean by -1.75 compared with
the base prior. Every prior appears in the result, sorted by name.

## What is calculated

For measurements `y` (there are `n`), a prior with mean `m0` and SD `s0`, and known
measurement SD `sigma`, the posterior variance is `v = 1 / (1/s0² + n/sigma²)` and the
posterior mean is `m = v * (m0/s0² + sum(y)/sigma²)`. The decision is true when the
posterior probability that the mean exceeds `--decision-threshold` is at least
`--decision-probability-threshold`.

`loo_elpd` adds up, over all measurements, the log density of each measurement predicted
from the posterior fitted to the other measurements, with predictive variance
`sigma² + v_loo`. Higher is better. The result does not give an uncertainty for the
difference between priors and does not pick a winner.

Differences are always alternative minus base:

- `conclusion_changed`: the alternative prior reaches a different decision.
- `material_mean_shift`: the posterior mean moved by at least `--material-mean-shift`.
- `decision_stable`: no alternative prior changed the decision. The mean can still move
  materially when the decision is stable, and stability across the priors you listed says
  nothing about priors or model assumptions you did not try.

Choose priors that are plausible for your scientific question. This is an experimental
sensitivity check.

## Rust API and limits

From Rust, call `marklab_bayes::evaluate_normal_mean_prior_sensitivity(spec,
timeout_seconds)` with a `NormalMeanPriorSensitivitySpec`. It returns a
`NormalMeanPriorSensitivityResult` and does not touch the filesystem. The CLI adds the input
paths and hashes of the parsed observations and priors.

Limits: 2 to 100,000 measurements; 2 to 32 priors with names up to 128 bytes; at most 3.2
million measurement–prior combinations; each CSV up to 16 MiB; output up to 1 MiB. The
probability cutoff must be strictly between 0.5 and 1, the materiality cutoff positive, and
the timeout between 1 and 3600 seconds. The timeout is checked between priors and before
returning. Invalid inputs, or numbers that cannot be represented, fail without writing
output, and an existing output file is never replaced.

## Output format

The output is a `marklab.bayesian_normal_mean_prior_sensitivity` document, **version 2**,
with `backend: "native_rust"`. Version 1 was computed by a Python (SciPy) worker. Version 2
keeps the same scientific fields and definitions but drops the Python version, environment
lock and worker fields; `request_sha256` identifies the sorted inputs and timeout. Programs
reading these files need to accept version 2 and its backend value. The old SciPy version 1
request and result types remain in `marklab-bayes` for compatibility and as an independent
reference check. This change does not affect result format 0.3 or the PyMC-based commands.
