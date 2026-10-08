# Classical spatial workflow

`marklab classical` asks whether cells are clustered or regularly spaced, using Ripley's K
and L functions. It reads the same cell tables as `marklab analyze` and keeps the same rows
(usable tumor cells with a usable stain reading), but it ignores the label: it looks at
where the cells are, not which ones are marked.

The observed curves are compared with simulations that scatter the same number of cells
completely at random inside the same tissue outline (complete spatial randomness, CSR). A
global envelope test decides whether the observed pattern falls outside what random
placement produces at any distance.

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

All arguments are required. `--r-max-um` must be positive and `--r-steps` between 1 and
4096; Marklab evaluates the curves at `r_i = r_max * i / r_steps` for `i = 1..r_steps`.
`--simulations` must be positive, `--alpha` must be between 0 and 1, and
`(simulations + 1) * alpha` must be at least 1 so that a significant result is possible.
Every resource limit must be positive.

The cell table can be any CSV or Parquet file that `marklab analyze` accepts, with
coordinates in micrometres. Unlike `analyze`, this command also accepts zero or one
retained cell, and reports those cases as having too few points instead of failing. Two
cells at the same position are an error.

## Tissue outline

The mask is a GeoJSON `MultiPolygon`, given as a Geometry, a Feature, or a FeatureCollection
with one feature. Limits: 16 MiB, 4,096 polygons, 65,536 rings, one million coordinate
positions (counting ring closures) and eight million topology checks. Rings must be
finite, closed, have nonzero area and no zero-length edges. Self-intersections, overlaps,
crossings, touching rings or polygons, holes outside their polygon, and nested or
overlapping polygons are errors. Marklab never repairs invalid geometry.

The outline is normalized (ring direction, starting point, hole and polygon order) so the
same shape always has the same identity. Points on an outer boundary or a hole boundary
count as inside; points strictly inside a hole are outside. The result records the area,
the total perimeter including holes, the bounding box, the polygon, hole, ring and position
counts, the coordinate convention, this boundary rule and a digest of the outline.

## Estimator

For window area `A`, `n` cells, distance `r`, `m(r)` cells at least `r` from every
boundary, and `q(r)` ordered pairs of distinct cells whose first cell is one of those
`m(r)` and whose separation is at most `r`, the border-corrected estimators are:

```text
K_border(r) = A q(r) / (n m(r))
L_border(r) = sqrt(K_border(r) / pi)
```

Only cells at least `r` from the edge serve as centers, so neighbors missing outside the
outline do not bias the counts. Pairs are counted by streaming over a spatial index, without
building an all-pairs matrix.

Each point on the curve reports `m(r)`, `q(r)`, the values expected under complete
randomness (`K = pi r²`, `L = r`), the observed K and L when defined, the envelope bounds
when the radius is eligible for the test, and an availability status. Undefined values
are left out instead of being written as NaN, infinity or a placeholder number.

## Null model and test

The null hypothesis is complete spatial randomness: the same number of cells, `n`, placed
uniformly at random inside the same outline. Each simulation draws random points in the
bounding box with a seeded generator (SplitMix64) and keeps those inside the outline. The
whole pattern is the unit being tested; individual cells are not independent replicates.
If the `--max-csr-draws` limit is reached before every simulation completes, the command
fails instead of running fewer simulations.

The global envelope test uses extreme rank lengths (ERL) on the L curves. A radius is
included only if the observed curve and every simulated curve have an L value there. The
result reports the global p-value, the ERL depths, the envelope, the number of simulations,
the seed, alpha, the number of cells and the number of radii tested.

## Results

The command runs through Marklab's workflow scheduler. `result.json` is a `marklab.classical_spatial` version
1 document, separate from result format 0.3. It records the input, the outline, the
settings and the identities used for caching.

The overall status is one of:

- `available`: at least one radius could be tested;
- `insufficient_points`: fewer than two cells; no simulations are run and no K or L
  values are reported;
- `insufficient_inference_support`: observed K and L may exist, but no radius is defined
  in every simulated curve.

The output directory contains exactly three files:

- `result.json`: the scientific result and run identity;
- `run_manifest.json`: input paths, all settings, the design, cache and output identities,
  and a list of the files;
- `report.md`: a plain summary of what was measured, the outline, the edge correction, the
  null model, the availability and what the result does and does not show.

The directory appears only after all three files are written. An existing non-empty
directory or a symbolic link as the target is rejected. Any failure leaves no output and
never overwrites an existing directory.

## What the result means

A significant result says the cells in this one section are clustered or more regularly
spaced than random placement within the outline would produce. It says nothing about which
cells are labeled, and it is not a statement about patients, populations, treatment,
causes or clinical meaning. For labels, cross-type patterns, varying density, patient
cohorts, Bayesian models or 3-D, see the other workflows in
[What Marklab can do](capabilities.md).
