# Task contract — BAY-GIBBS-PL-01 Strauss pseudolikelihood fit

Status: complete

Date: 2026-08-25

Parent requirements: BAY-02, BAY-PP, BAY-PP-B, WS-43, BAY-BT-01, BAY-STRAUSS-STAT-01.

## User outcome

`marklab bayes fit-strauss-pseudolikelihood` fits a typed inhibitory Strauss conditional-intensity
pseudolikelihood on nested exact rectangle grids through pinned SciPy.

## Frozen behavior

- consume exact `point_id,x_um,y_um` inside a finite positive-area half-open micrometre rectangle,
  positive radius, nested coarse/fine regular grids, explicit positive beta bounds, gamma bounds in
  `(0,1]`, bounded optimizer/neighbor/table work, timeout, and fresh output;
- at each resolution put every observed point and one midpoint dummy in its cell, split exact cell
  area equally, set observed response `1/weight` and dummy response zero, and compute IC-0071
  inclusive-radius neighbor count excluding the point itself for observed nodes;
- fit `log lambda=log(beta)+neighbor_count*log(gamma)` by weighted Poisson pseudolikelihood through
  pinned SciPy 1.18.1 under exact log bounds. Report exact gradient/Hessian, model covariance, and
  2x2-window-block sandwich SE plus coarse/fine parameter/objective sensitivity;
- Rust revalidates table weights/features and every likelihood/derivative/result identity. Do not
  call pseudolikelihood a normalized likelihood or posterior, and do not hide boundary/quadrature
  approximation or interpret gamma as causal interaction.

## Validation and claims

A deterministic inhibited-grid fixture must produce complete bounded coarse/fine fits with positive
beta, gamma in the declared interval, finite positive SEs, improved objective over the optimizer
start, exact window-weight sums, and finite refinement differences. This validates numerical and
table mechanics only—not parameter calibration, likelihood equivalence, biology, causality, or
clinical use.
