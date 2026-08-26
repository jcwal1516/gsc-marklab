# Task contract — BAY-STRAUSS-STAT-01 Strauss statistic and Papangelou intensity

Status: complete

Date: 2026-08-25

Parent requirements: BAY-PP, BAY-PP-B, WS-43.

## User outcome

`marklab bayes strauss-statistics` computes exact fixed-radius Strauss sufficient statistics and the
Papangelou insertion intensity for one proposed point under declared `beta,gamma` parameters.

## Frozen behavior

- consume exact `point_id,x_um,y_um` CSV with 0–100,000 unique nonempty IDs and finite coordinates,
  positive finite interaction radius, finite proposal coordinate distinct from every existing point,
  positive finite beta, finite gamma in `[0,1]`, a positive pair-visit cap, and fresh output;
- `n` is exact point count; `s_R` counts unordered existing pairs with Euclidean distance `<=R`;
  insertion delta counts existing points at distance `<=R` from the proposal; Papangelou intensity is
  exactly `beta*gamma^delta`, including gamma-zero conventions `0^0=1` and `0^positive=0`;
- bound exact pair work before execution, retain input identity, radius/inclusive boundary convention,
  counts, proposal, parameters, delta, intensity, and claim ceiling. Reject non-finite overflow rather
  than saturating;
- do not call this a fitted Strauss model, normalized likelihood, process simulation, or adequacy test.

## Validation and claims

For points `(0,0),(3,4),(10,0)` at radius `5`, exact unordered pair count is one. Proposal `(4,0)`
has insertion delta two and `beta=2,gamma=0.5` gives Papangelou intensity `0.5`. This validates exact
fixed-pattern mechanics only—not inference, simulation convergence, biology, causality, or clinical use.
