# Task contract — EMB-CROSS-COV-01 embedding cross-covariance by distance

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, MRK-02D, SIG-01G, SIG-01H, WS-32, WS-50,
EMB-VARIO-01.

## User outcome

`marklab bayes embedding-cross-covariance-by-distance` computes exact undirected embedding
cross-covariance matrices by physical distance, with rotation-invariant summaries.

## Frozen behavior

- consume the same finite unique object/coordinate/ordered-vector contract and contiguous physical
  bin policy as EMB-VARIO-01;
- compute one stable global embedding mean over all declared rows, visit every unordered pair once,
  and accumulate `outer(embedding_i - mean, embedding_j - mean)` in `f64` under declared pair and
  matrix-element work bounds;
- divide each nonempty bin matrix by its pair count and symmetrize it as `(C + C^T) / 2`, making the
  undirected result invariant to pair orientation. Empty-bin matrices are typed unavailable;
- report pair counts, trace, and Frobenius norm as low-dimensional orthogonal-rotation invariants and
  retain the complete ordered feature names, global mean, physical bins, and full matrices in a
  separately typed matrix-artifact section of the output;
- retain exact input/bin identities. This is an unweighted descriptive cross-covariance, not a
  co-located cross-modal statistic, edge-corrected estimator, null test, patient-level comparison,
  biological interpretation, or completion of broader EMB-01/WS-50.

## Validation and claims

A three-object two-dimensional hand fixture must reproduce the exact one-pair symmetrized matrix,
trace, and Frobenius norm, preserve the shared-edge bin assignment, and retain equal invariant
summaries after an orthogonal rotation of every complete vector.
