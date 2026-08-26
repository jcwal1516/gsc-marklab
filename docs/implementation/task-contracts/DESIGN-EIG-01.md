# Task contract — DESIGN-EIG-01 Gaussian nested-Monte-Carlo information gain

Status: complete

Date: 2026-08-25

Parent requirements: Part XII §114.1 `EstimateExpectedInformationGain`, ACT-01, WS-84.

`marklab causal gaussian-eig` consumes a named scalar candidate design, finite Gaussian prior mean,
positive prior/noise standard deviations, finite nonzero design sensitivity, positive outer/inner
sample counts, deterministic seed, and maximum likelihood evaluations. For each outer draw it samples
`theta~N(prior_mean,prior_sd)`, simulates `y=sensitivity*theta+Normal(0,noise_sd)`, evaluates the
generating log likelihood, draws an independent inner prior bank, and subtracts the stable log mean
likelihood. It retains every outer information value, mean, Monte Carlo SE, exact analytic Gaussian
EIG `0.5*ln(1+sensitivity^2*prior_variance/noise_variance)`, signed/absolute bias, and exact
`outer*(inner+1)` likelihood work under a named ChaCha20 namespace.

For unit prior/noise/sensitivity the analytic EIG is `ln(2)/2`; the bounded Monte Carlo estimate must
agree within its reported uncertainty and replay byte-identically. This is an analytically checked
scalar utility estimator, not sequential acquisition, ROI/stain/landmark selection, or operational
laboratory guidance.
