# Task contract — LONG-PARTICLE-01 bootstrap particle filter and ancestry smoother

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §99.3, DIM-01, WS-81.

`marklab longitudinal particle-smooth` consumes a bounded scalar initial Gaussian particle law,
time-varying analytic quadratic transitions/observations with Gaussian noise, nullable observations,
a particle count, ESS resampling fraction, smoothed-trajectory count, deterministic seed, and maximum
particle-step work. It uses sequential log weights, stable normalization, systematic resampling, and
retains complete post-resampling particles, weights, ESS, ancestry, likelihood increments, and
resampling status at every step.

The ancestry smoother samples terminal particles by final weights and traces the stored ancestor
indices backward. A deterministic zero-variance unit-drift oracle must return exact filtering means
and `[1,2,3]` trajectories. This finite bootstrap specialization does not establish particle-count
adequacy, general proposal optimality, backward-simulation smoothing, spatial-field inference, or
biological validity.
