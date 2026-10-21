# Bayesian Thomas cluster model

**Research.** `marklab bayes advanced-cluster` fits a Bayesian Thomas (Neyman–Scott)
cluster process to one or more cell patterns, one pattern per patient, and pools the
patients through a hierarchical prior. `marklab bayes advanced-cluster-sbc` runs
simulation-based calibration (SBC) of the same sampler. Both need the
[Python environment](python-backends.md).

## Model

Each pattern `j` has a rectangular window `W_j` in micrometres. Parents form a Poisson
process with intensity `κ_j` (parents per µm²) on `B_j`, the window widened by the declared
`dilation_um` on every side. Each parent has a Poisson number of offspring with mean `μ_j`,
displaced from it by an isotropic Gaussian with standard deviation `σ_j` (µm). Only offspring
inside `W_j` are observed. Given the parents, the observed cells therefore form a Poisson
process with intensity `μ_j Σ_c N₂(u; c, σ_j² I)`, and the likelihood uses the exact window
probability of each parent's offspring, a product of normal CDF differences.

The log parameters are pooled across patterns:
`(log κ_j, log μ_j, log σ_j) ~ Normal(η_p, τ_p²)` independently, with
`η_p ~ Normal(m_p, s_p²)` and `τ_p ~ HalfNormal(h_p)`. The input declares `m_p`, `s_p` and
`h_p` for each parameter. The output reports each pattern's posterior and, for each
parameter, the population median `exp(η_p)` and the between-pattern SD `τ_p`.

Parents are allowed only in `B_j`, the usual bounded-region approximation for cluster
processes (Møller and Waagepetersen 2004). Parents beyond `B_j` would still send offspring into the window; the result reports
the exact share of offspring intensity they would contribute, `edge_truncation_fraction`,
which for a window of width `w` and height `h` is `1 − (1 − d_x/w)(1 − d_y/h)` with
`d = 2σ(L(R/σ) − L((w+R)/σ))` and `L` the standard normal loss function. A fit whose largest
posterior truncation share exceeds `maximum_edge_truncation_fraction` is non-converged.

## Sampler

Each chain alternates, for every pattern:

1. spatial birth–death proposals for the parents (Geyer and Møller 1994). Births are uniform
   on `B_j` or near a randomly chosen cell, with the proposal density in the Hastings ratio;
   deaths remove a uniformly chosen parent. Half of the proposals also rescale `μ_j` so that
   the expected number of offspring in the window is unchanged; that rescaling is a
   translation of `log μ_j` with unit Jacobian, reversed by the matching move;
2. a Gaussian displacement proposal for every parent;
3. random-walk Metropolis updates of `log κ_j`, `log μ_j` and `log σ_j`.

The hierarchy then receives a Gibbs update of each `η_p`, a centred move that shifts `η_p`
and every `log` parameter together, a non-centred move that rescales `τ_p` and the
deviations from `η_p` together, and a centred `τ_p` update. Step sizes adapt only during
warmup. Every chain is seeded from the request seed, so a request reproduces its result.

Convergence uses ArviZ 1.3.0 rank R-hat and bulk and tail ESS over all chains, for every
pattern's `κ`, `μ` and `σ` and every `exp(η_p)` and `τ_p`. The fit is `complete` only when the
worst R-hat, bulk ESS and tail ESS meet the declared `diagnostic_policy`, the parent capacity
was never reached and the edge truncation is within policy. Otherwise `fit_state` is
`nonconverged`, `nonconvergence_reasons` lists every failed criterion and `claim_status` is
`diagnostic_only_nonconverged`: the summaries are retained for diagnosis, not as posterior
results.

## Limits and checks before sampling

The input declares the parent capacity per pattern, the maximum points per pattern, the
kernel memory and a work bound. Marklab computes the kernel storage (current and proposed
point-by-parent matrices) and an upper bound on kernel evaluations, `chains × sweeps × Σ n_j ×
(birth–death proposals + 8 × parent capacity)`, and refuses the request before the worker
starts if either exceeds its declaration. The worker repeats both checks. Points must lie
in their window, and pattern and patient identifiers must be unique.

## Posterior predictive checks

For evenly spaced posterior draws the worker simulates fresh patterns and reports, for each
pattern, the share of replicates with at least as many cells as observed and with a mean
nearest-neighbour distance at least as large. These two summaries do not test every aspect
of the model.

## Evidence

- **Known answers** (`tests/python/test_bayesian_thomas_math.py`): the incremental
  likelihood equals the direct Thomas likelihood after hundreds of moves; the closed-form
  edge truncation equals numerical integration within 1e-7; and, with `κ` and `σ` fixed and
  one observed cell, integrating out the parents gives `p(log μ | x) ∝ N(log μ) μ A(μ)
  exp(κ E(μ))` and `E[m | μ] = 1 + κ E(μ)`. Four chains reproduce the quadrature means of the
  parent count and `log μ` within four Monte Carlo standard errors, a tolerance fixed before
  the comparison.
- **Recovery** (`tests/bayes_advanced_cluster_cli.rs`): on three patterns simulated by
  Marklab's independent Rust Thomas simulator, every true `σ` and offspring intensity `κμ`
  lies inside its central 95% posterior interval.
- **SBC:** 200 replicates of the full hierarchy, two patterns per replicate (500 µm square
  windows, 100 µm dilation; median 103 cells per pattern, range 20–240), 2 chains × 1,000
  draws after 400 warmup, ranks of 49 thinned draws in 10 bins. The criteria, fixed before the
  run, were rank-uniformity p ≥ 0.001 and 90% coverage in [0.84, 0.96] for each of nine
  quantities: the population mean, between-pattern SD and first pattern's value of `log κ`,
  `log μ` and `log σ`. All nine passed (p 0.078–0.905, coverage 0.860–0.920) with no
  parent-capacity hits. 114 prior draws were redrawn because a simulated pattern fell outside
  the declared 20–250 cells; that rule depends only on the data, so it preserves rank
  uniformity. 165 of the 200 replicates missed the replicate convergence policy (R-hat ≤ 1.05,
  bulk ESS ≥ 100; median worst R-hat 1.040, median worst ESS 52), so the ranks come from
  autocorrelated draws and the result supports, rather than establishes, calibration.
- **spatstat `kppm`:** [`compare_kppm.R`](../tests/fixtures/bayesian_thomas/compare_kppm.R) fits the
  `redwood` pattern (62 points, spatstat.data) and three Schürch et al. B-cell spots
  (CC BY 4.0; 259, 319 and 587 cells, chosen by count before any fit) and runs
  spatstat.model 3.7-2 `kppm` Thomas fits by minimum contrast, Palm likelihood and composite
  likelihood. spatstat (GPL) runs only as an external oracle. These are different estimators,
  so the comparison is descriptive and has no tolerance. In the first run (4 chains × 2,000
  draws) every Marklab fit was non-converged (worst R-hat 1.03–1.30). In a second, disclosed
  run with 4 × 8,000 draws, one B-cell spot converged; `redwood` (R-hat 1.012, bulk ESS 154),
  the other two spots (1.019 and 1.031) and the joint three-spot fit (1.016) remained
  non-converged under R-hat ≤ 1.01 and ESS ≥ 400. Their posterior summaries changed little
  between runs. For `redwood`, every kppm estimate lies inside the 95% posterior interval;
  ratios to the posterior median are 0.83–1.18 for `κ`, 0.92–1.31 for `μ` and 1.07–1.40 for
  `σ`. For the B cells, minimum contrast and Palm likelihood describe fewer, larger clusters
  (`μ` 59–169, `σ` 33–61 µm) than the posterior (`μ` 17–24, `σ` 29–38 µm), and most kppm
  estimates lie outside the posterior intervals. The posterior predictive mean
  nearest-neighbour check gives upper-tail probabilities of 0.95–1.00: the B cells sit closer
  together than the fitted Thomas model predicts. A single-scale Gaussian cluster model does
  not describe these follicles well, so neither set of estimates should be read as their true
  cluster size.

## Format

The result has format `marklab.bayesian_thomas_cluster_model`, version 1. Earlier
`advanced-cluster` outputs, format `marklab.advanced_cluster_and_gibbs_models` version 2
(a k-means cluster-count heuristic, a finite binary Gibbs exchange fit and fixed shrinkage),
are no longer produced; `marklab_bayes::advanced::thomas_cluster::ClusterModelDocument`
reads either format. The finite binary Gibbs exchange fit remains available as the optional
`finite_gibbs` input; it is independent of the cluster model. SBC results have format
`marklab.bayesian_thomas_cluster_sbc`, version 1.

## What this does not establish

Cells are not literally offspring of hidden parents. The model is a descriptive account of
clustering at one scale with Gaussian clusters, no background cells and no covariates, in
rectangular windows. A converged posterior on synthetic data does not show that tissue
follows this model, and the kppm comparison is descriptive.
