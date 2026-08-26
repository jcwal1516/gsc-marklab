# Task contract — BAY-GIBBS-SIM-01 bounded Strauss birth/death simulation

Status: complete

Date: 2026-08-25

Parent requirements: BAY-PP, BAY-PP-B, WS-43, BAY-STRAUSS-STAT-01.

## User outcome

`marklab bayes simulate-strauss-birth-death` runs a deterministic bounded birth/death Metropolis
chain for the exact rectangle Strauss density and retains convergence-relevant trace diagnostics.

## Frozen behavior

- require a finite positive-area half-open micrometre rectangle, positive finite beta, gamma in
  `[0,1]`, positive finite radius, 100–100,000 iterations, burn-in below iterations, seed, 1–10,000
  point cap, 1–100,000,000 neighbor-visit cap, and fresh output; initialize at the empty pattern;
- each iteration chooses birth/death with equal probability. Birth proposes uniformly in the exact
  window and accepts `min(1, area*lambda(u|x)/(n+1))`; death chooses one existing point uniformly and
  accepts `min(1, n/(area*lambda(x_i|x\i)))`. Empty death is a retained null transition;
- use IC-0071 inclusive-radius Papangelou mechanics and a domain-separated pinned ChaCha20 stream.
  Retain every count, proposal/acceptance/null/resource total, post-burn first/second-half means and
  drift, final exact-ID pattern, and all parameters/seed;
- abort on point/visit overflow or non-finite acceptance arithmetic. Label the finite terminal state
  experimental MCMC simulation, not an exact independent process draw or converged sample.

## Validation and claims

For gamma one on a `100x100` window with beta `0.01`, a 20,000-iteration/5,000-burn seeded chain must
be byte-repeatable, keep every point inside the window and below cap, have nonzero birth/death
acceptances, and show a post-burn count mean compatible with the Poisson special-case expectation 100
under a declared deterministic tolerance. This validates mechanics/special-case behavior only—not
general mixing, fitted adequacy, biology, causality, or clinical use.
