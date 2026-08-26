# Task contract — BAY-BT-01 Berman–Turner rectangle-grid refinement

Status: complete

Date: 2026-08-24

Parent requirements: BAY-PP, WS-43, BAY-IPP-LIKE-01.

## User outcome

`marklab bayes berman-turner-refinement` constructs observed-plus-dummy weighted Poisson tables on
coarse and nested fine rectangle grids and reports fixed-parameter objective convergence.

## Frozen behavior

- reuse IC-0062 exact half-open rectangle/events and two complete midpoint covariate/offset grids;
  coarse/fine x/y dimensions are 1–1,024 with at most 100,000 output nodes each, and fine dimensions
  are integer multiples with strictly more cells;
- within each cell, create one dummy midpoint node plus every observed event assigned to that cell.
  Split exact cell area equally across its nodes. Observed pseudo-response is `1/weight`, dummy is
  zero; weights are positive, nodes are deterministic, and total weight equals window area;
- for finite fixed intercept/coefficient, weighted Poisson objective without parameter-independent
  constants is `sum weight*(response*eta-exp(eta))`. Retain coarse/fine objectives, absolute change,
  caller positive convergence tolerance, and fine table;
- constant covariate/offset makes both resolutions algebraically equal to IC-0062 exact event term
  minus integral. General covariates remain an approximation and convergence is empirical only;
- strict paths/content/window/grid/node/event identities, complete coverage, finite arithmetic,
  bounded output/work, and experimental approximation claim ceiling are retained.

## Validation and claims

The two-event constant-intensity IC-0062 fixture on coarse `2x1` and fine `4x2` grids must give
weight sums 2, objectives `2*log(2)-4`, zero refinement change, a converged flag, and a ten-node fine
table. This validates Berman–Turner construction/refinement mechanics only—not arbitrary-window
accuracy, fitted convergence, process adequacy, biology, causality, or clinical evidence.
