# Task contract — CAUSAL-INTERFERENCE-01 exact randomized binary interference

Status: complete

Date: 2026-08-25

Parent requirements: Part XII §§103–104, CAU-01, WS-82.

`marklab causal randomized-interference` consumes a provenance-labelled randomized design with
unique units, explicit independent assignment clusters, observed binary treatment, finite outcome,
strict treatment-before-outcome times, and finite baseline covariates measured no later than
treatment. Every unit is eligible in this first specialization. A prespecified unique undirected
interference graph defines `BINARY_ANY_TREATED` neighbour exposure. Each cluster declares a treated
count strictly between zero and cluster size; the observed assignment must conform.

Marklab exactly enumerates the Cartesian product of within-cluster complete-randomization states
under caller state and unit-state work limits. It derives every unit's exact probability for the four
joint `(own treatment, any treated neighbour)` exposures. For each positive-probability target it
returns HT and, when observed denominator is positive, Hájek means plus the exact fixed-outcome
rerandomization SD. Prespecified direct effects at fixed neighbour exposure, spillover effects at
fixed own treatment, and the `(1,1)-(0,0)` total contrast are computed only when both Hájek means
exist.

The randomization test uses the HT contrast between caller-declared joint exposures, samples from
the actual enumerated assignment mechanism under a named ChaCha20 stream, and reports an inclusive
plus-one two-sided p-value with every null value. A four-unit single-cluster line with two treated
units has exactly six assignment states. This validates randomized-design mechanics only; it does
not admit observational identification, unknown assignment, post-treatment adjustment, causal
mediation, or real treatment-effect claims.
