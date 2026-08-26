# Task contract — EMB-KERNEL-01 training-fitted kernel mark correlation

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, SIG-01H, WS-32, WS-50, EMB-VARIO-01.

## User outcome

`marklab bayes kernel-mark-correlation` fits an embedding kernel only on training biological units,
freezes it, and computes raw/normalized similarity curves separately for each declared split.

## Frozen behavior

- consume 8–10,000 unique finite objects with exact biological-unit and train/validation/test split,
  micrometre coordinates, 2–128 ordered `embedding_*` columns, and contiguous physical bins; each
  biological unit belongs to exactly one split and train plus at least one held-out split are required;
- fit the feature center on training rows only. Linear uses centered dot product; cosine uses centered
  nonzero vectors and normalized dot product; RBF freezes the median positive training-pair Euclidean
  distance as bandwidth; Laplacian freezes the median positive training-pair L1 distance as scale;
- label all four kernels positive semidefinite under these exact formulas and retain the complete
  preprocessing/kernel/training-unit artifact. Learned kernels remain outside this bounded workflow;
- for each split compute the complete unordered-pair global kernel reference and the raw mean kernel
  over each eligible physical-distance bin. Normalize only when the global reference exceeds caller
  tolerance; otherwise return typed `zero_global_kernel_reference` unavailability, never NaN;
- enforce a declared bound covering kernel-fitting and split pair visits; retain exact input/bin,
  feature, kernel, scale, split, pair-count, and reference identities. This is descriptive and has no
  random-label p-value, patient-level comparison, real-asset admission, or biological interpretation.

## Validation and claims

A two-unit RBF fixture must freeze training-only center `[1.5,0]` and bandwidth `1.5`, reproduce the
three-bin hand kernel means/reference/normalization, and remain unaffected in its artifact by extreme
held-out values in the second embedding dimension.
