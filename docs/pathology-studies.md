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
rather than cells are the unit of comparison. These workflows are at the Research tier,
except that `spatial-study` is Experimental for fixed-bandwidth L-minus-r curves in
rectangular windows (see [capabilities](capabilities.md#command-maturity)).

```sh
marklab pathology composition --recipe examples/pathology-studies/composition.json --out output/composition
marklab pathology spatial-study --recipe examples/pathology-studies/spatial-study.json --out output/spatial-study
marklab pathology scan --recipe examples/pathology-studies/scan.json --out output/scan
```

The output directory must be new or empty. From Rust, `analyze_pathology_composition`,
`analyze_pathology_spatial_study` and `analyze_pathology_scan` take the recipe as JSON bytes
and return the result; the matching `publish_*` functions write the output directory. These
workflows write their own versioned result documents (v1), separate from
[result format 0.4](result-format-0.4.md).

## Recipes

Each recipe is a JSON file with `format`, `version: 1`, `study`, `design` and `limits`. The
formats are `marklab.pathology_composition_recipe`, `marklab.pathology_spatial_study_recipe`
and `marklab.pathology_scan_recipe`. Unknown fields or versions are errors. The files in
`examples/pathology-studies/` are complete synthetic examples; `generate.py` in the same
directory recreates them.

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

The Gaussian model and its simulations condition on the observed cell count. If a slide
has `n` independently located cells with location density `f`, this is a binomial point
process with first-order intensity `lambda(u) = n f(u)`. At an observed cell, a
leave-one-out kernel sum has only `n-1` training cells and estimates `(n-1) f(u)`, after
smoothing and edge correction. Marklab therefore multiplies the sum by `n/(n-1)` to
estimate the declared `n`-cell intensity. More generally, a held-out fold uses
`n / training_count`. The fixed-count model is also named in the result as
`fixed_gridded_inhomogeneous_binomial`, and its randomization unit says that the whole
location pattern is conditioned on count.

This convention would be different for an unconditional Poisson process. By the
[Mecke-Slivnyak theorem](https://stoch.math.kit.edu/img/Last/lastpenrose2017.pdf), removing
the conditioned point leaves a Poisson process with the original intensity, so the raw
leave-one-out sum is the Poisson-model estimator. `spatstat` documents that raw sum in
[`density.ppp`](https://search.r-project.org/CRAN/refmans/spatstat.explore/html/density.ppp.html).
Changing Marklab to that convention would change its model and serialized scientific
meaning, rather than merely improve numerical agreement.

The distinction also affects the reference curve. A fixed-`n` binomial process has
same-type pair correlation `g = (n-1)/n`, as derived in
[Baddeley's point-process notes](https://leman.stat.vt.edu/VTCourses/BaddeleyPointProcesses.pdf),
so its corresponding K curve is `(n-1)/n` times the Poisson curve away from window effects.
The `theoretical_k = pi r^2` and `theoretical_g = 1` fields are Poisson reference curves;
they are not the finite-count simulation null. The global-envelope inference uses the
declared fixed-count simulations. For cross-type g, the source and target patterns are
independent conditional on their separate type counts, and their cross pair correlation
remains 1. The type-specific `n/(n-1)` factors therefore use the same intensity definition
for L, g and directed cross-g.

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
or filled with zeros.

### Sparse cell types

By default a radius bin is supported only when the observed curve and every simulated curve
have an estimate there. For `g` and cross-type `g`, eligible centers with positive normalization
but no pairs in kernel support give a zero estimate. A bin without eligible centers remains
unavailable and can leave a slide without a common supported distance axis. A design may also
declare, before analysis,

```json
"support_rule": {"kind": "expected_pair_count", "minimum_expected_pairs": 1.0,
                 "maximum_draws_per_simulation": 50}
```

for Gaussian intensity models. Its expected pair count at radius `r` is
`centers(r) × partners / area × A(r)`: the observed eligible centers (border distance at
least `r`, or `r + h` for `g`), the partner count (`n − 1`, or the target count for cross-type
`g`) over the window area, and the area a pair must fall in (`πr²` for L; the kernel annulus
`π((r + h)² − max(0, r − h)²)` for `g`). It uses counts and border distances only, never pair
distances or curve values. If any declared radius falls below the minimum, the slide is
unavailable and the reason names that radius; bins are still never removed. Otherwise:

- a simulated pattern without eligible centers at a radius where the observed pattern has
  them is redrawn from a seed derived from that simulation's own seed. Observed and accepted
  simulated patterns then satisfy the same support event, so the Monte Carlo test is
  conditional on it. After `maximum_draws_per_simulation` draws the slide is unavailable.

The result reports each slide's `expected_pairs` and `support_redraws`. Without the rule,
results are unchanged. Patient groups are compared with a functional L2 test that shuffles
whole patients. Three different p-values appear in the results and answer different
questions: the within-slide global envelope test, the Bonferroni combination across slides,
and the patient-group comparison.

Outputs: `result.json`, `patient-curves.csv`, `slide-curves.csv`, `curves.svg`,
`null-curves.json` and `report.md`, including density diagnostics, which null was used, and
anything unavailable.

`null-curves.json` (format `marklab.pathology_spatial_null_curves`, version 1) holds, for each
slide, the observed curve and every simulated curve of its within-slide envelope test, on the
reported scale (`L(r) − r` in µm, or `g(r) − 1`), with the slide seed, null model and the
envelope's own p-value. Simulated values are written only at radii that were eligible in the
observed curve and in every simulated curve, which are exactly the radii the ERL envelope
used; other entries are `null`. A slide whose analysis failed is listed with its reason and no
curves. The file is separate from `result.json`, whose contents are unchanged, and its size
is charged to the study's memory budget before publication.

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
Every scenario must produce inference in at least 95% of attempts, except
`sparse_cross_g_refit` (below). A null scenario fails if the lower bound of its Wilson interval
exceeds the nominal 0.05 level; the interval uses a Bonferroni-adjusted z over the 15 null
scenarios, so a correctly sized method fails the run with probability at most 2.5%. An
alternative fails if the lower bound of its 95% Wilson interval for the rejection rate is
below 0.8. The experiment evaluates every scenario before failing and reports all failed
criteria together.

`sparse_cross_g_refit` (4 immune sources among 40 cells, cross-type `g` at 100 and 200 µm)
and `cluster_g_refit` (two tight clusters, `g`) declare the expected-pair support rule with a
minimum of 1 and 50 draws per simulation. Without it every attempt lacked a complete radius
axis. A uniformly placed source is an eligible center at 200 µm with probability
`0.52² = 0.2704`, so the sparse scenario can support the axis in only `1 − 0.7296⁴ = 0.717` of
attempts; its availability threshold was fixed at 0.65 before the run, and size is assessed
among available attempts. In the 10 October 2026 run, the sparse null scenario produced
inference in 693 of 1,000 attempts (the other 307 named radius 200 or 100 µm, with no eligible
source), with 32 rejections: **0.046** (Wilson 95% 0.033–0.065). The clustered alternative
rejected in all 250 attempts (Wilson 0.985–1.000). Every other scenario reproduced the
previous run's counts exactly. The weekly calibration workflow runs it and saves its JSON lines,
including partial output when the run stops early.
An executed comparison with spatstat and GET is described below. The separate
[Schürch](validation-schurch.md) and [Jackson](validation-jackson.md) studies validate
the common patient Max-T workflow on real groups. They do not validate patient inference
from this command's fitted spatial curves or scan statistics.

## Reference comparisons on published cells

On 10 October 2026, the production `pathology spatial-study` command was compared with
`spatstat.explore` 3.8-3 on the published
[rabbit amacrine](https://search.r-project.org/CRAN/refmans/spatstat.data/html/amacrine.html)
and [cat beta ganglion](https://search.r-project.org/CRAN/refmans/spatstat.data/html/betacells.html)
cell recordings in `spatstat.data` 3.1-9. Both datasets have documented physical scales.
The comparison used the original observation rectangles and coordinates in micrometres,
with all cells, off cells and on cells evaluated separately: six cases and 96 radius values.
These are two specimen recordings, not six independent subjects.

The executable comparison is
[`compare_spatstat.R`](../tests/fixtures/inhomogeneous_spatial/compare_spatstat.R). It writes
recipes, command outputs, per-radius comparisons and `summary.json` to a new external
directory. It requires an existing Marklab executable and R libraries `spatstat.geom`,
`spatstat.explore`, `spatstat.data`, `GET` and `jsonlite`; these are optional validation
tools, not application dependencies. R and the complete package dependency set are fixed
in [`reference-packages.tsv`](../tests/fixtures/inhomogeneous_spatial/reference-packages.tsv),
and the script rejects any other versions. For example, with a matching local R library:

```sh
R_LIBS=target/reference-r-library Rscript \
  tests/fixtures/inhomogeneous_spatial/compare_spatstat.R \
  target/debug/marklab <output-dir>
```

The checks distinguish four calculations:

- **Pair estimator:** supply Marklab's event intensities to
  [`Kinhom`](https://search.r-project.org/CRAN/refmans/spatstat.explore/html/Kinhom.html)
  with `correction="border"` and `renormalise=FALSE`. The largest
  `abs(K_marklab - K_reference) / max(1, abs(K_reference))` was **2.59e-11**, below the
  **1e-10** tolerance. The native L-minus-r transformation also passed its separate
  **1e-10 µm** absolute tolerance. Raw reference values are retained, including tiny
  floating-point residuals of either sign at zero-pair radii.
- **Intensity estimation:** compare the 64×64 grid approximation with an independent
  Gaussian calculation using the analytic rectangular-window mass from the normal CDF,
  including the same `n/(n-1)` training-count multiplier. The maximum relative intensity
  difference was **0.0475%**, below the **0.1%** tolerance. Recomputing spatstat K from
  those independent intensities gave a maximum scaled difference of **0.0425%**, below
  the **0.2%** tolerance. The initial comparison omitted the multiplier and failed;
  matching that estimator definition resolved the discrepancy without changing Marklab
  or relaxing the tolerances.
- **Convention comparison:** the runner supplies both raw and `n/(n-1)`-scaled analytic
  intensities to `Kinhom`, each with `renormalise=FALSE` and with the default first-power
  renormalisation enabled. For the border ratio estimator, scaling every intensity by
  `n/(n-1)` scales K by `(n-1)/n`, not its square, because one inverse-intensity factor
  cancels in the eligible-center denominator. The largest algebra error was **5.59e-11**
  against a prespecified **1e-10** tolerance. With `renormalise=TRUE, normpower=1`,
  `Kinhom` multiplies by `area(W) / sum(1/lambda_i)`, so the global intensity scale cancels;
  the largest invariance error was **5.12e-11**. These definitions are documented in the
  [`Kinhom` manual](https://search.r-project.org/CRAN/refmans/spatstat.explore/html/Kinhom.html).
  The runner also executes the actual package-default pipeline, with intensity omitted,
  leave-one-out enabled and renormalisation enabled. Its scaled difference from Marklab
  ranged from **4.18%** to **21.1%** across the six cases. That difference is descriptive:
  it combines a different intensity convention and implementation with `Kinhom`'s
  data-dependent renormalisation, and it has no agreement threshold.
- **Global envelopes:** GET **1.0-9** exactly reproduced the existing checked-in tied-curve
  oracle: depths, critical depth, lower/upper bounds and p-value **0.8**. The four native
  ERL tests also passed, including identical curves, radius eligibility and finite-sample
  envelope coverage.
- **Within-slide null curves:** for each of the six cases the runner passes the exported
  `null-curves.json` curves to GET 1.0-9 `global_envelope_test(type = "erl", alternative =
  "two.sided")` on the eligible radii (all 16 here). Tolerances, fixed before the run, were
  1e-12 absolute for the p-value and 1e-12 scaled for the envelope bounds. All six p-values
  agreed (largest error **2.08e-17**) and the bounds agreed within **5.08e-15**. Every
  observed retinal curve was the most extreme of its 40 curves (p = 1/40 in both), so these
  cases test only the extreme rank. A prespecified synthetic pattern drawn from an
  inhomogeneous Poisson process (`rpoispp`, intensity `2e-4 exp(1.5 x / 1000)` per µm² in a
  1000 µm square; 454 cells) gave a non-extreme p-value, **0.25** in both, with identical p-values and bounds (error
  0). Run 01 had six cases and run 02 added the synthetic case; all other existing
  checks passed again.

The real-data runs use a fixed bandwidth of 15% of the shorter window side and 16 radii
from 1.25% to 20% of that side. Their 39 simulations exercise the production pipeline;
they are not a calibration or power experiment. The GET agreement covers the plug-in
fixed-intensity `L(r) − r` null; refitted, `g`, cross-type `g` and compartment nulls export
the same file but were not compared. No patient
replication or group labels were invented, and patient-group inference correctly remained
unavailable. The scheduled calibration workflow is configured to rerun this pinned
comparison weekly and retain its generated recipes, curves and summary as an artifact.
That job has not yet run in CI; the recorded results come from local macOS runs.

The comparison does not claim `g` or cross-type `g` parity. Marklab's pathology estimators
use a radius-plus-pair-bandwidth border eligibility rule and normalize by the eligible
inverse-intensity center sum. The native
[`pcfinhom`](https://search.r-project.org/CRAN/refmans/spatstat.explore/html/pcfinhom.html)
and
[`pcfcross.inhom`](https://search.r-project.org/CRAN/refmans/spatstat.explore/html/pcfcross.inhom.html)
interfaces report translation and Ripley-isotropic estimates, not that border-ratio
estimator. Treating one as the other would manufacture agreement between different
statistics. A future package-parity comparison needs a genuinely corresponding production
edge correction; the existing independent rectangle oracles continue to test Marklab's
present formulas. Fitted-null calibration, irregular observation windows, real-cohort
inference from these fitted curves and clinical interpretation also remain unvalidated. The two unsupported
calibration scenarios above remain support-handling tests.
