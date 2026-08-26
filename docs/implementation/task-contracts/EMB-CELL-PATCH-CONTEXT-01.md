# Task contract — EMB-CELL-PATCH-CONTEXT-01 overlap-aware weighted cell patch context

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PATCH, FR-02, FR-02A, WS-51, C-05.

## User outcome

The public `marklab-embeddings::cell_patch_context` API computes one cell's weighted patch-context
vector from the existing canonical C-05 link/table/overlap artifacts.

## Frozen behavior

- reuse C-05 `CellPatchLink` as the canonical owner of `ValidateCellPatchLinks`: exact cell/patch IDs,
  one context/scale, finite positive declared weights, vector-free shared references, physical support,
  and link identity are already validated by its constructors and artifact receipts;
- require exact owning-slide, expected-patch ID/digest, patch-footprint ID/digest, and table/link/overlap
  bindings before reading vectors; find the exact requested canonical `CellId` and return typed missing
  modality for an unavailable/empty assignment;
- require every linked patch vector present and complete. Normalize equal weights for contained-shared
  links or the exact rational weights for declared interpolation, then accumulate the weighted context
  vector in stored component order using `f64`;
- map every linked patch to the existing positive-area overlap graph component, sum normalized weights
  per dependency group, and report overlap-aware Kish effective count
  `1 / sum(group_weight^2)` plus raw linked-patch and dependency-group counts;
- enforce a declared linked-patch × dimension operation bound and retain table/link/overlap/context
  identities. This is within-cell descriptive aggregation, not independent-patch inference, source
  correspondence proof, prediction, biology, or clinical evidence.

## Validation and claims

For declared weights `[1/2,1/4,1/4]`, patch vectors `[0,0]`, `[4,0]`, `[0,8]`, and overlap groups
`{a,b}`/`{c}`, the exact context must be `[1,2]` and the dependency-aware effective patch count `1.6`.
