# Task contract — SBI-SYNTH-01 growth-front synthetic likelihood and MCMC

Status: complete

Date: 2026-08-25

Parent requirements: BAY-SBI, GEN-01, WS-73.

## User outcome

`marklab bayes synthetic-likelihood-growth-front` estimates a shrinkage Gaussian likelihood for two
noisy growth-front summaries and consumes it in bounded random-walk MCMC with complete retained
likelihood state and trace diagnostics.

## Pseudocode ownership

This workflow owns the two-summary, declared-Gaussian-observation-noise growth-front specializations
of `EstimateSyntheticLogLikelihood` and `SyntheticLikelihoodMCMC`. General summaries, adaptive
replicate allocation, unbiased likelihood correction, non-Gaussian synthetic likelihood, and real
calibration remain separate work.

## Frozen behavior

- use final total density mass and maximum final density with caller-declared positive independent
  Gaussian observation-noise scales;
- for every likelihood evaluation, run 8–10,000 canonical simulator replicates, add named-seed
  observation noise, estimate the two means and unbiased sample covariance, shrink only off-diagonal
  covariance by a declared `[0,1]` amount, and require positive determinant;
- retain covariance, determinant, log Gaussian density, per-summary Monte Carlo mean SE, replicate
  count, and shrinkage;
- run a bounded Gaussian random-walk chain under a finite uniform prior, retaining the current noisy
  likelihood estimate on rejection, every chain state, burn-in, acceptance, and sample posterior;
- require 10–100,000 iterations and at most 250 million declared simulator cell-steps over the
  initial plus proposed likelihood evaluations;
- label the posterior approximate and synthetic-only, never calibrated biological inference.

## Validation and claims

With 32 replicates, 1,500 iterations, 500 burn-in, and the analytic logistic summaries mass one and
maximum density `0.5`, the posterior mean recovers growth rate one within `0.05`, retains 1,000
draws, has acceptance strictly between `0.05` and `0.95`, and replays byte-identically from seed
`20260825`.
