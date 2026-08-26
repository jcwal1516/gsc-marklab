# Task contract — EMB-CROSS-MODAL-COV-01 cross-modal covariance by distance

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, SIG-01G, SIG-01H, MM-01, WS-32, WS-50, WS-53,
EMB-CROSS-COV-01.

## User outcome

`marklab bayes cross-modal-covariance-by-distance` computes weighted A–B embedding covariance from
an explicit source-section/compartment-bound pair plan and performs stratified random-label inference.

## Frozen behavior

- consume two exact finite modality tables with 1–128 ordered `embedding_*` dimensions and stable
  object/source-section/compartment identities, 1–256 contiguous physical bins, and 1–1,000,000
  unique declared A–B pairs with positive finite weights;
- require every pair's source section and compartment to agree exactly with both referenced objects;
  the pair plan, rather than implicit row order or fabricated coordinates, owns eligibility, bin, and
  weight;
- center each modality using either one declared global mean or declared compartment-stratified means,
  then accumulate weighted rectangular outer products in `f64` and normalize by exact bin weight sum;
- report full rectangular matrices and Frobenius norms, which are invariant under independent
  orthogonal rotations of A and B;
- random-label complete B vectors within exact source-section × compartment strata for 20–10,000
  deterministic permutations. Studentize Frobenius norms and return single-step max-T adjusted
  p-values over nonempty physical bins; degenerate null cells remain descriptive and unavailable;
- enforce declared pair and component-pair-visit bounds and retain all input/pair/bin, seed, policy,
  and permutation identities. This synthetic workflow does not establish cross-modal correspondence,
  registration quality, patient-level inference, biology, causality, or clinical evidence.

## Validation and claims

A two-dimensional four-pair hand fixture must return near matrix identity, far matrix diagonal
`[-1,1]`, Frobenius `sqrt(2)` for both bins, complete max-T metadata, and equal invariant summaries
after independently rotating both modality vectors.
