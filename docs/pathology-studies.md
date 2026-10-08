# Pathology studies

Three workflows for comparing patients. All run in Rust; no Python or R is needed.

- **`composition`** compares cell-type proportions and densities between patient groups,
  and checks whether the result changes with the size and placement of the grid used to
  count cells.
- **`spatial-study`** measures how cells are arranged, such as clustering of one cell type
  or how often type A sits near type B, and compares those curves between patient groups.
- **`scan`** searches each slide for the circular region most enriched in a cell type and
  tests whether a region that extreme could arise by chance.

Every cell has one definite type, and every slide names its patient, so that patients
rather than cells are the unit of comparison. These workflows are experimental.

```sh
marklab pathology composition --recipe examples/pathology-studies/composition.json --out output/composition
marklab pathology spatial-study --recipe examples/pathology-studies/spatial-study.json --out output/spatial-study
marklab pathology scan --recipe examples/pathology-studies/scan.json --out output/scan
```

The output directory must be new or empty. From Rust, `analyze_pathology_composition`,
`analyze_pathology_spatial_study` and `analyze_pathology_scan` take the recipe as JSON bytes
and return the result; the matching `publish_*` functions write the output directory. These
workflows write their own versioned result documents (v1), separate from
[result format 0.3](result-format-0.3.md).

## Recipes

Each recipe is a JSON file with `format`, `version: 1`, `study`, `design` and `limits`. The
formats are `marklab.pathology_composition_recipe`, `marklab.pathology_spatial_study_recipe`
and `marklab.pathology_scan_recipe`. Unknown fields or versions are errors. The files in
`examples/pathology-studies/` are complete examples.

The `study` section names the study, lists the cell types in order, and records how they
were measured and where the data came from. It then lists the slides. Each slide has a
`slide_id`, `patient_id`, `group`, `coordinate_frame_id`, an observation window (a GeoJSON
MultiPolygon) and its cells. Each cell has an `id`, `x_um`, `y_um`, a `phenotype` (cell type)
and a `stratum`.

Rules:

- Coordinates are in micrometres, finite, distinct and inside the window. Cell IDs are
  unique within a slide.
- A patient's group must be the same on all of their slides.
- Cell types must be definite labels. Probabilities cannot be rounded into labels here.
- Slides with no cells stay in the study as empty slides.
- Windows keep their holes and separate pieces. If the window is the union of sampled
  patches, it describes what was observed, not the shape of the tissue.
- You are responsible for removing duplicate or overlapping sampling, and for choosing
  observation windows and strata that make sense for your question.
- Cell types predicted by a model remain predictions. The examples are synthetic.

The `limits` section caps total cells, total work, polygon vertices, grid tiles or scan
candidates (`maximum_tiles` covers both) and estimated memory. Recipes are limited to
64 MiB. Scan limits assume the worst case for each query. For spatial studies, each
slide's work budget is split between intensity estimation, pair counting and simulation; a
slide that fails still uses up its budget, and building compartments counts toward the
geometry and memory limits. These limits protect the machine; they are not performance
benchmarks.

## Composition and density

For each patient, cell counts and observed area are pooled across their slides, and every
patient counts equally in group comparisons. Density is cells per mm². Fractions give the
relative abundance of each cell type. Observed area with no cells still counts as area with
zero cells; a patient with no cells at all has no composition.

Two log-ratio summaries compare groups of cell types:

- `geometric_balance`: `sqrt(a*b/(a+b))` times the difference between the mean log counts
  of two non-overlapping groups of cell types, of sizes `a` and `b`.
- `amalgamated_log_ratio`: the log ratio of the two groups' total counts.

These ratios do not change when all counts are scaled by the same factor. If any count
involved is zero, the value is unavailable; Marklab does not add pseudocounts.

### Grid sensitivity (MAUP)

Counting cells in grid squares can give different answers depending on the square size
and where the grid starts (the modifiable areal unit problem, MAUP). The recipe names a
cell type, a grid origin, a primary grid, and a set of named square widths and offsets
(each offset between 0 and the width). Every square is clipped to the observation window.
For each grid, Marklab computes the area-weighted variance of that cell type's density
across squares, around the patient's overall density. Empty squares with positive area
count. Every cell and every bit of area is assigned exactly once.

A cell on the lower or left edge of a square belongs to that square. If the window boundary
puts a cell in a square with zero area inside the window, the cell goes to the first
neighboring square with positive area, checking left, below, then below-left. The 100, 200
and 400 µm widths in the example are illustrations, not recommended values.

### Inference

All available densities, balances and grid results are tested together as one Max-T
family across patients. If a measurement is missing for any patient, it is left out of the
group test instead of dropping that patient. A measurement that is identical for every
patient cannot be tested. If the test fails for any other reason, the whole family is
reported as failed instead of trying again with different permutations.

Bootstrap intervals resample patients within groups. They hold for each measurement on its
own, not for all of them at once. The share of grids that came out significant is a
description, not a combined p-value.

Outputs: `result.json`, `patients.csv`, `contrasts.csv`, the clipped grids as GeoJSON,
`maup.svg` and `report.md`. Results show how much the effect changes with grid offset and
whether its sign flips compared with the primary grid.

## Spatial curves

Choose one measurement:

- `l_minus_r`: Besag's L(r) − r for one cell type. Above zero means clustered, below zero
  means regular spacing.
- `g_minus_one`: the pair correlation g(r) − 1, the same question at each distance.
- `cross_g_minus_one`: g(r) − 1 for a source cell type relative to a different target
  cell type.

Real tissue is not uniformly dense, so the curves are measured relative to local density
(intensity). The recipe either estimates that density with a Gaussian kernel (`plugin` or
`refit` below) or uses tissue compartments (`binary_compartments`). Gaussian settings are
the bandwidth, the integration grid, a minimum intensity, and optional cross-fitting folds
keyed by stable cell IDs. For `l_minus_r` only, you can give increasing bandwidth candidates
and Marklab picks one by leave-one-out likelihood, without cross-fitting. The bandwidth is
never chosen by looking at the curves or p-values. An optional cell-type filter applies to
L and g. Cross-type g needs two different cell types. Edge effects use standard border
correction.

How the null hypothesis handles the density estimate:

- `plugin` keeps the density estimated from the real data.
- `refit` (experimental) re-estimates the density, and reselects the bandwidth if enabled,
  for every simulated pattern. This alone does not guarantee correct false-positive rates.
- `binary_compartments` replaces the density estimate with two tissue compartments (for
  example tumor and stroma). It requires `conditional_compartments`, exact geometry for
  both compartments on every slide with its source, and compartments that tile the window
  exactly. It supports `l_minus_r` and `g_minus_one`, and holds the number of cells in each
  compartment fixed. Sparse compartments and unclear boundaries are reported as
  unavailable.

Each patient's curve is the area-weighted mean of their slide curves, on one distance axis
that every slide supports. Distances, slides and patients are never dropped, interpolated
or filled with zeros. Patient groups are compared with a functional L2 test that shuffles
whole patients. Three different p-values appear in the results and answer different
questions: the within-slide global envelope test, the Bonferroni combination across slides,
and the patient-group comparison.

Outputs: `result.json`, `patient-curves.csv`, `slide-curves.csv`, `curves.svg` and
`report.md`, including density diagnostics, which null was used, and anything unavailable.

## Enrichment scans

Name the cell type to look for, which cells are eligible, the circle radii to try, and the
inference settings. Every eligible cell is a candidate circle center. A circle must have
at least two eligible cells inside and two outside, and at most half of them inside.

Each circle gets a score: the sum over strata of the Bernoulli log-likelihood ratio for a
higher rate of the target cell type inside the circle than outside. A stratum with no
inside-outside contrast adds zero. Strata can differ in their baseline rate and in how
strongly the target type is enriched.

To test the best circle, Marklab shuffles labels within each stratum, keeping each
stratum's number of target cells, and reruns the whole search each time. The p-value is
`(1 + number of shuffles scoring at least as high) / (B + 1)`, and a Bonferroni correction
covers all slides in the study. Ties go to the first center in canonical order, then the
smallest radius. The best circle is always reported, even when it is not significant. If
labels are constant within every stratum, or no circle qualifies, the scan is unavailable.
Relative enrichment needs at least one target cell outside the circle.

Cells are assigned to circles by exact distance. Plots draw circles as 128-sided polygons
clipped to the window and report the largest approximation error. A circle marks a search
region, not a lesion boundary, and a circle spanning two tissue pieces does not mean they
are connected. Only the best circle is tested; secondary clusters and population-level
questions are not.

Outputs: `result.json`, `scans.csv`, `strata.csv`, an SVG and GeoJSON per slide, and
`report.md`.

## Testing and calibration

The `pathology_studies` test target checks the numbers and geometry against hand-computed
answers, covering ordering, patient pooling, zeros and missing data, bandwidth selection,
cross-fitting, compartments, an exhaustive six-cell scan compared with the exact null,
strata, limits and the three CLI commands.

A separate calibration experiment, `pathology_studies_prespecified_calibration` in
`tests/pathology_studies_calibration.rs`, is skipped in normal test runs. It uses fixed
seeds and generators: 1,000 outer replicates per null scenario, 250 per alternative, and
199 inner replicates. Composition simulates 16 independent patients and counts any
rejection in the Max-T family. The spatial and scan experiments check a single
within-slide global p-value; they do not show calibration of patient curves or of the
across-slide combination in a real cohort. Plugin and refit use the same simulated
patterns. Large spatial runs use fixed bandwidths and leave-one-out estimation; the
cross-fitting and bandwidth-selection options are covered only by the focused tests.

To run it, set `MARKLAB_STAT_CALIBRATION_OUTPUT` to a new file path and run the ignored test
in release mode. Each JSON line reports attempted, available, unavailable and error counts,
rejection rates, Wilson intervals and Monte Carlo uncertainty. An unavailable result is
never counted as a successful non-rejection, and an unexpected error fails the experiment.
Comparison with the R packages spatstat and GET, and validation on real studies, are still
to be done.
