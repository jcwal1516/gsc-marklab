# Classical spatial-pathology workflow

`marklab classical` is the end-to-end homogeneous Ripley K/L workflow. It uses
the existing CSV/Parquet cell adapters and their tumor/IHC row filtering, but
the estimand is the retained unmarked location pattern. Mark values do not enter
K, L, CSR simulation, or ERL inference.

## Command

```bash
marklab classical \
  --cells cells.parquet \
  --mask window.geojson \
  --out out/classical \
  --r-max-um 100 \
  --r-steps 50 \
  --simulations 999 \
  --seed 123456789 \
  --alpha 0.05 \
  --memory-budget-mib 512 \
  --max-pair-visits 100000000 \
  --max-csr-draws 10000000
```

All listed arguments are required. `r-max-um` must be finite and positive;
`r-steps` must be in `1..=4096`. Marklab evaluates
`r_i = r_max * i / r_steps` for `i = 1..=r_steps`. `simulations` is positive,
`alpha` is finite in `(0, 1)`, and `(simulations + 1) * alpha` must be at least
one. Every resource limit is positive.

The cell input may be any CSV or Parquet table accepted by `PatternLoader`.
The classical adapter uniquely permits zero or one retained row so these cases
reach the typed scientific availability boundary; the compatibility `analyze`
adapter retains its existing nearest-neighbor requirement. Coordinates use the
existing physical x/y micrometre convention. Duplicate retained coordinates,
including signed-zero aliases, are rejected.

## Exact observation window

The mask is UTF-8 GeoJSON containing a `MultiPolygon` in a Geometry, Feature,
or one-feature FeatureCollection. Input is bounded at 16 MiB, 4,096 polygon
components, 65,536 rings, 1,000,000 positions including ring closures, and
8,000,000 topology candidates. Rings must be finite, closed, nonzero-area, and
free of zero-length edges. Self-intersections, overlaps, crossings, touching
rings/components, holes outside an exterior, and nested or overlapping
components are errors; invalid topology is never repaired.

Valid ring orientation, start position, hole order, and component order are
canonicalized for identity. Window membership is closed: exterior and hole
boundaries are included, while hole interiors are excluded. The result records
area, total exterior-plus-hole perimeter, bounds, component/hole/ring/position
counts, the physical coordinate convention, membership policy, and canonical
digest.

## Estimator and geometry plan

One exact reusable geometry plan owns the point R-tree, boundary-segment index,
per-point boundary distances, canonical point/window identity, and streaming
ordered-pair traversal. It does not retain an all-pairs matrix.

For window area `A`, `n` retained points, radius `r`, `m(r)` centers at least
`r` from every window boundary, and `q(r)` ordered distinct pairs whose center
is eligible and whose distance is at most `r`, the reported standard-border
estimators are:

```text
K_border(r) = A q(r) / (n m(r))
L_border(r) = sqrt(K_border(r) / pi)
```

Each curve point includes `m(r)`, `q(r)`, theoretical `K = pi r²` and `L = r`,
observed K/L when available, ERL bounds when jointly eligible, and an explicit
availability status. Undefined values are absent rather than NaN, infinity, or
a numeric filler. Exact zero is serialized as positive zero.

## Null and inference

The only admitted null is homogeneous CSR conditional on the observed `n` and
the fixed exact window. A deterministic domain-separated SplitMix64 stream
draws uniform bounding-box candidates and retains those in the window. The
randomization unit is the whole location pattern; individual cells are not
independent biological replicates. The command either completes the exact
requested simulation count or returns a typed CSR-draw-limit error.

The existing extreme-rank-length global-envelope implementation operates on L
curves. A radius is inference-eligible only when the observed curve and every
simulated curve have an available L value there. The result reports the global
p-value, ERL depths, envelope, simulation count, seed, alpha, conditioned point
count, and eligible-radius count.

## Results and availability

The command executes one `ClassicalSpatialAnalysisNode` through the existing
local project scheduler. Cells, canonical window, radii, simulations, null,
seed, alpha, resource limits, implementation, codec, and scheduler limit all
participate in cache identity. `result.json` is a strict
`marklab.classical_spatial` version-one document containing input/window,
geometry, configuration, private node-artifact, and scheduler-cache identities.
It is not a result-format 0.3 document.

Top-level status is one of:

- `available`: at least one radius supports joint ERL inference;
- `insufficient_points`: fewer than two retained points; no CSR simulation is
  attempted and all K/L values are absent;
- `insufficient_inference_support`: observed K/L may exist, but no radius is
  available across every simulated curve.

The output transaction writes exactly:

- `result.json`: strict typed scientific result and workflow identity;
- `run_manifest.json`: input paths, complete controls, scientific design,
  cache/output identities, and artifact inventory;
- `report.md`: estimand, window, correction, null, whole-pattern unit,
  availability, and claim boundary.

The final directory is renamed into place only after all three files are
written. A non-empty or symbolic-link target is rejected. Any input,
computation, codec, resource, or write failure leaves no successful output and
does not overwrite an existing non-empty directory.

## Interpretation boundary

The workflow tests departure from conditional homogeneous CSR for one retained
section-level location pattern. It does not establish a biological mechanism,
patient/population effect, treatment effect, causality, clinical validity,
clonality, or mark-specific interaction. It provides no inhomogeneous, cross,
marked, pair-correlation, F/G/J, cohort, Bayesian, 3-D, or longitudinal
analysis; those remain separate master-plan workstreams.
