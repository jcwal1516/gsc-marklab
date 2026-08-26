# Task contract — REG-FGW-01 entropic fused Gromov-Wasserstein alignment

Status: complete

Date: 2026-08-25

Parent requirements: FR-03B, REG-01C, BACK-01.

## User outcome

`marklab bayes fused-gromov-wasserstein` computes a bounded descriptive alignment that combines
scaled feature cost and scaled within-support structural distortion.

## Frozen behavior

- consume 1–32 strictly positive probability-mass supports, complete finite feature vectors,
  finite nonnegative symmetric zero-diagonal structure matrices, explicit feature/structure scales,
  alpha, epsilon, tolerance, iteration limit, and timeout;
- use pinned POT 0.9.7.post1 `entropic_fused_gromov_wasserstein` with squared feature distance,
  squared structural loss, and the pseudocode convention that alpha is the feature weight;
- run independent-mass, feature-EMD, and structure-profile-EMD initializations and preserve every
  resulting plan and convergence diagnostic;
- independently replay in Rust every marginal, feature term, four-index structural term, entropy,
  regularized objective, fit state, and best-plan choice;
- classify all output as descriptive alignment, not physical correspondence or a unique map.

## Validation and claims

For two equal-mass supports with reversed scalar features and isometric two-point structures, the
best plan assigns more than `0.9` total mass to the feature-reversed isometry and reports all three
initializations.
