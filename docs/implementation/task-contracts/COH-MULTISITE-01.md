# Task contract — COH-MULTISITE-01 multisite patient-effect inference

Status: complete

Date: 2026-08-25

Parent requirements: COH-01, BAY-03, BAY-HIER-A, INF-01D, WS-34, WS-41.

`marklab cohort multisite-inference` consumes at least three site-specific patient-level effects,
positive standard errors, and patient counts. Fixed effects use inverse-variance pooling. Random
effects maximize the intercept-only restricted likelihood over nonnegative between-site variance
and report a normal prediction interval. Both branches retain site interaction Cochran Q/chi-square
diagnostics and refit after omitting every site. The hand fixed-effect oracle pools `1,2,3` with unit
SE to `2`, SE `1/sqrt(3)`, Q `2`, and exact leave-one-out means; a package oracle requires positive
REML heterogeneity for `0,3,6` with SE `0.5`. Input effects remain summary estimates, so this does not
substitute for one-stage hierarchical fitting or prove transportability.
