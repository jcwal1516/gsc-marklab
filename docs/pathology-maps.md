# Pathology maps

`marklab pathology maps` answers three questions about each slide:

- How do marker levels and cell density change with distance from an annotated structure,
  such as a tumor border or a vessel?
- What mix of cell types surrounds each cell, at several neighborhood sizes?
- Where are the local hotspots and outliers for a single marker such as Ki67?

It works from cell measurements and annotations you provide. It does not segment tissue
or predict IHC from H&E. This workflow is experimental.

```sh
cargo +1.99.0 run --locked --package marklab --no-default-features --features cli -- \
  pathology maps --recipe examples/pathology-maps/recipe.json --out target/pathology-maps-example
```

The output directory must be new or empty. The four-cell example is synthetic, including
its Ki67 values and cell types, and its distances are chosen so the results can be checked
by hand. They are not recommended radii.

For each slide the output contains `result.json`, `report.md`, a local-Moran SVG, one
neighborhood SVG per radius, and `slide-NNN-maps.geojson` with the per-cell results as map
layers. GeoJSON coordinates are in the micrometre frame you supplied, not longitude and
latitude or raw image pixels. To overlay results on a slide image, apply the recorded
coordinate transform first.

From Rust:

```rust,ignore
let result = marklab::analyze_pathology_maps(&std::fs::read("recipe.json")?)?;
marklab::publish_pathology_maps(&result, std::path::Path::new("maps"))?;
```

## Recipe

Start from the example recipe (version 1). Unknown fields are errors.

- **`marker`**: one numeric or yes/no measurement, with its unit, how it was measured and
  its source. The measurement status is one of `measured`, `imported_prediction`,
  `morphology_prediction` or `derived_summary`, and is carried into the result. Write `null`
  for a missing value. Category codes are not numeric markers.
- **`phenotypes`**: the cell types in order, with how they were assigned and their source.
  Every cell gives a probability for each type, in that order, summing to 1 (within
  `1e-6`). For definite labels use a 1 and zeros. The workflow never derives cell types
  from marker thresholds or embeddings.
- **Slides**: each has its real patient ID, unique slide and coordinate-frame IDs,
  calibrated coordinates, an observation window (a GeoJSON MultiPolygon) and any annotated
  structures (also MultiPolygons). An empty `annotations` list runs the neighborhood and
  hotspot maps and returns empty distance profiles. For distance profiles, the window must
  be the actual tissue outline. For maps without annotations, the window can instead be the
  union of sampled patches, with its source recorded in the recipe; that window does not
  say where tissue ends inside the patches. Windows can have holes and separate pieces.
  Cells must lie inside the tissue.
- **Cells**: IDs are unique within a slide and appear in results as `slide_id::cell_id`.
  Cells, annotations and slides are sorted by ID so runs are reproducible.
- **Annotations**: each records its source and a boundary uncertainty in micrometres.
  Distance bands narrower than twice that uncertainty are rejected. This is only a check on
  the inputs; the uncertainty is not carried through to the estimates.
- **`stratum`**: the group within which marker values may be shuffled for the hotspot test.
  Give every cell the same stratum only if values could plausibly be swapped anywhere on
  the slide. Cells with a missing marker, and cells with no neighbors that have a marker
  value, are left out of the test and reported as unavailable.

## Distance profiles around annotations

Distance to the annotated structure is negative inside it and positive outside, including
around holes. A cell on the boundary has distance zero. Bands include their lower edge and
exclude their upper edge, except the last band, which includes both. Cells beyond the
outermost band are left out.

Each band reports the total cells, how many have the marker observed or missing, the mean
observed marker value, the expected count of each cell type (the sum of its
probabilities), and densities per mm². A cell with a missing marker still counts toward
density.

Band area is the difference between successive buffers around the annotation, each clipped
to the tissue window. Buffers are polygons with a 0.05-radian angular step, so curved band
areas are approximate. The result records the largest error this causes,
`max(|distance|)*(1-cos(0.025))`. Cells are assigned to bands by exact distance, so densities
in thin or intricate bands can be sensitive to this approximation. A band with zero area has
no density, and a cell that lands in a band with zero computed area is an error.
Overlapping annotations are analyzed separately; do not add their counts as if they were
separate pieces of tissue.

## Neighborhood maps

For each radius you specify (positive and increasing), every cell gets a summary of the
cells within that radius, excluding itself: the number of neighbors, the mean probability
of each cell type, the effective diversity `exp(-sum(p*ln(p)))`, and its distance to the
edge of the window.

A neighborhood is labeled with a dominant cell type when that type's share reaches the
threshold you set (which must be above 0.5); otherwise it is mixed. A label needs at least
the minimum number of neighbors and a full circle inside the window. Cells near the edge or
with too few neighbors still appear with their composition but get no label.
`distance_to_tissue_edge_um` measures distance to the edge of the window you supplied; with
a sampled-patch window that is the edge of the sampled area, not the edge of the tissue.

These labels describe the cell composition you supplied. They are not learned or validated
tissue niches.

## Local hotspots and outliers

The hotspot test computes local Moran's I for one marker on the graph of cells that have a
value: `I_i = z_i * mean(z_neighbors)`, where `z` is standardized with the population
variance. Other software often uses the sample variance, so match the normalization before
comparing values. Cells with no neighbors, too few observed points, or a constant marker
get no p-value.

The null shuffles every marker value within its stratum, including the value of the cell
being tested. This is random labeling on fixed cell positions, not the conditional
permutation test common in LISA tools such as GeoDa and PySAL, which holds the tested
cell's value fixed.

P-values count the observed value as one of the permutations. They are adjusted for testing
every cell on the slide using the maximum absolute statistic, then multiplied by the number
of slides and capped at 1. The correction covers one marker at one radius across the slides
in the run. Running more markers or radii separately adds tests to your study, and you must
correct for those yourself.

Every available cell reports its quadrant (high-high, low-low, high-low, low-high). Only
cells that pass the adjusted significance level are labeled `high_high_hotspot`,
`low_low_coldspot`, `high_low_outlier` or `low_high_outlier`. These are statements about
one slide, not about patients or populations, because cells are not independent patients.
The distance profiles and neighborhood maps are descriptive and have no p-values.

## Limits

A recipe can be at most 64 MiB, with up to 64 slides, 64 cell types, 16 neighborhood radii
and 16 annotations per slide. The optional `limits` section can change the defaults for
total cells, neighbor-pair visits, permutation work, polygon vertices and geometry work, and
estimated memory. Work is counted across the whole run, and hitting a limit stops the run
before anything is written. The default permutation budget is 50 million edge evaluations.
For full slides you can raise the limits to at most one million cells, 250 million pair
visits, 50 billion permutation edge evaluations and 8 GiB of estimated memory.

If several slides together exceed the limits, run each slide separately and apply the
correction across the full set of slides yourself. Analyzing tiles separately is not the
same as analyzing the whole slide, because the neighbor graph and the correction change.
The limits protect the machine; they are not performance measurements.

## Testing

Tests use rectangular and holed tissue with hand-computed answers, exact enumeration of
small permutation sets, planted marker clusters and outliers, missing values, invariance to
large coordinate offsets, ordering, limit errors, and agreement between the library, the CLI
and the written output. Validation on real annotated H&E/IHC data, calibration across a
study, propagation of boundary uncertainty and stability of neighborhoods across patients
still need their own data and analyses.
