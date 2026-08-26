# Task contract — EMB-PATCH-DEPENDENCY-01 overlap-component patch weighting

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PATCH, FR-02, FR-02A, WS-51, C-05.

## User outcome

The public `marklab-embeddings::patch_dependency_weighting` API converts positive patch aggregation
weights into an overlap-component-aware descriptive effective patch count.

## Frozen behavior

- consume one or more unique exact patch IDs with finite positive within-specimen weights and the
  existing C-05 positive-area `PatchOverlapGraph`; reject unknown or duplicate patches;
- normalize weights over supplied patches, aggregate normalized mass by exact connected overlap
  component, and compute Kish effective count `1 / sum(component_weight^2)` with compensated `f64`
  sums;
- retain raw patch count, dependency-group count, effective count, overlap logical identity, and the
  explicit `patient_not_patch` inferential-unit policy;
- use these weights only for within-specimen aggregation/descriptive uncertainty. They never make
  patches independent biological replicates and do not supply patient-level inference.

## Validation and claims

Patch weights `[1/2,1/4,1/4]` over overlap groups `{a,b}` and `{c}` must yield normalized group mass
`[3/4,1/4]`, two dependency groups, and effective count `1/(9/16+1/16)=1.6`.
