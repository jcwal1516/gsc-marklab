# Validation methodology

Marklab checks its statistics in two ways.

**`marklab smoke`** runs the real analysis pipeline on synthetic data where the right
answer is known: patterns with no structure, which must not be flagged, and patterns with
planted structure, which must be. It is quick and catches a broken pipeline.

**Scheduled calibration** runs 1,000 synthetic datasets with no structure through the
same engines every week and checks that the share flagged as significant is not
significantly higher than the stated false-positive rate. That is the evidence that p-values
are not anti-conservative. Neither
check shows clinical validity or fitness for diagnosis.

## How the scenarios are built

Every scenario runs synthetic input through the same code as the real commands.
Single-section scenarios build a `Pattern` and call `AnalysisEngine`. Multimodal scenarios
build H&E cells, IHC cells, landmarks, metadata and an `AnalysisConfig`, then call
`MultimodalEngine::analyze_run`. Before/after scenarios analyze both inputs and use the
same comparison functions as `marklab prepost`. The generators only create inputs: they
never write result flags, build expected results, or force a scenario to pass.

## What each scenario reports

- replicates attempted, completed, and failed;
- exact failure reasons;
- the observed criterion rate and a two-sided 95% Wilson interval;
- endpoint-specific rates where applicable;
- the fixed smoke acceptance criterion;
- the base seed and permutation-seed policy;
- relevant configuration and the crate version.

A replicate that errors is counted in `replicates_failed` and fails the scenario. It is
never dropped or counted as a negative result.

## Multimodal scenarios

The quick suite covers these controls:

| Class | Scenario | What is checked |
| --- | --- | --- |
| Negative | random labels with no association | adjusted neighborhood-enrichment p-value |
| Negative | unrelated MMR-abnormal territories | separate territory clusters |
| Negative | immune cells independent of MMR territory | adjusted enrichment p-value |
| Negative | registration jitter without association | enrichment remains negative after fitted registration |
| Negative | matched pre/post organization | descriptive-margin result |
| Positive | related MMR-abnormal territories | one density-connected territory |
| Positive | immune-enriched MMR territory | adjusted enrichment p-value |
| Positive | cross-interaction enrichment | global cross-curve p-value |
| Positive | changed pre/post organization | descriptive-margin result |
| Registration | residual above configured maximum | engine rejection |
| Registration | association below registration resolution | graph edge-resolution flag |
| Edge | too few landmarks | input-validation error |
| Edge | degenerate landmarks | rigid-fit error |
| Edge | empty H&E or IHC section | fused-cell summary |
| Edge | no abnormal cells | available empty territory result |
| Edge | sparse graph | graph has no edges |
| Edge | zero expected edge count | typed unavailable enrichment ratio |
| Edge | multiple cell classes | every configured enrichment pair is present |
| Edge | multiple null models | every configured sensitivity result is present |
| Transform | known rigid rotation | fitted rigid coefficients |
| Transform | known affine deformation | fitted affine coefficients |

The before/after margin is descriptive: falling within it is not called equivalence,
and a non-significant pooled-bin p-value is not used as evidence of equivalence.

The "changed" before/after control keeps every cell where it is and swaps how the H&E
labels are arranged. That changes how the labels sit relative to each other while keeping
the distance bins comparable. If a geometry change leaves a distance bin undefined at one
time point, the comparison is reported as unavailable, not as an observed zero.

## Single-section scenarios

Single-section controls cover random labels, clustered labels with one or several
centers, directional patterns, dispersed labels, density and staining artifacts, loss of
internal-control staining, fragmented tissue, rare cell types, and before/after metadata
that do not match. Each is judged on the real outputs: spectra, residual territories,
anisotropy, QC flags, suppression status or comparison flags. The internal-control scenario
changes the internal-control fraction, not the overall retained fraction, and the metadata
mismatch flag comes from the real comparison function.

## Smoke acceptance

The quick suite uses deliberately strong, seeded controls. Every scenario must finish
without an unexpected error and meet its expected outcome in at least 80% of replicates
(90% for the multimodal random-label control). These thresholds catch a badly broken
pipeline; they are not error-rate guarantees.

## Bayesian simulation-based calibration

The NumPyro SBC runs for Gaussian and Student-t patient hierarchies, the four
beta-binomial hierarchy/regression models, the Dirichlet-multinomial group model,
and gridded LGCP use the same conservative replicate rule. Each simulated data
set must have exactly one disposition: a completed fit with ranks, or a failure
with a reason. A fit that misses the requested R-hat, bulk or tail ESS, E-BFMI,
divergence, or tree-depth policy is a failure and contributes no ranks. Any
failure makes the overall calibration nonconverged, even when the completed
fits pass the rank-uniformity and interval-coverage bounds. Replicates are not
retried or silently removed from the calibration outcome.

Rust checks the reported sampler diagnostics against the request, recomputes
rank histograms, uniformity p-values and coverage from completed rows, and
checks the complete disposition and final fit state. The original Gaussian
hierarchy result does not contain E-BFMI; its worker checks that gate before
emitting ranks. Its JSON fields remain unchanged, so Rust cannot independently
recheck that one diagnostic for this model.

## Continuous integration

Every pull request runs two test jobs. The Rust job runs the native tests. The backend job
installs Python 3.12 from the lock file, runs `marklab backend doctor`, runs the full
all-feature test suite two tests at a time, and runs the Python tests in `tests/python`
and `workers/python`. Python-backed tests are required in that job; they are never skipped
because an environment is missing.

## Scheduled calibration

The calibration workflow (`.github/workflows/calibration.yml`) runs weekly, or on demand,
outside pull-request CI. Its shuffle-calibration job runs the production `analyze` and
`multimodal` engines on synthetic data whose labels satisfy each test's null hypothesis by
construction, and on declared alternatives.

**`analyze`.** Labels are placed at random, at a fixed count, on fixed cell positions. The
engine uses the shipped configuration, including its 64-shell spectrum and validation
minimums, with 39 permutations instead of 999, a fresh permutation seed per replicate and
one thread. Five layouts at 20 µm spacing meet the shipped minimums of 200 cells and
100,000 µm²: a 16 × 16 square lattice, a 64 × 4 strip, a 272-cell ring with a hole, 256
cells in sixteen clusters (redrawn every replicate), and a 40 × 40 lattice. Each is run at
10%, 25% and 50% marked with 2,000 replicates, and the square is also run at 25% with 199
permutations. Power is checked with small foci of about nine cells (Gaussian, standard
deviation one spacing) at 25% marked, 500 replicates per layout. The tested statistic is
the primary endpoint, the one-sided low-k spectral excess.

**`multimodal`.** H&E lymphocyte classes and IHC MMR-abnormal marks are both placed at
random, at fixed counts. Two layouts: the original 5 × 5 H&E lattice at 15 µm with 12 IHC
cells, and a jittered 8 × 8 lattice at 9 µm with 24 uniformly placed IHC cells. Each is run
at 48% lymphocytes with 50% abnormal marks and at 20% with 25%. Every run is read under all
four source-section null models, with 2,000 runs per setting and 99 permutations. Power is
checked with the existing immune-association scenario under every null model, 500 runs.

**Acceptance, fixed before the runs.** A scenario fails if fewer than 95% of attempts
return a p-value. A null scenario fails when the lower end of its Wilson interval for the
rejection rate exceeds 5%, using a Bonferroni-adjusted z across the 16 null scenarios of
each workflow (z = 2.955). An exactly calibrated test then fails the run with probability
at most 2.5%. Requiring the upper end to be below 5% would instead reject an exactly
calibrated test most of the time. An alternative fails when the lower end of its 95%
Wilson interval for power is below 0.8. Rows are written as JSON lines and saved as a
workflow artifact.

**Results of 10 October 2026** (macOS, release build, run 04):

| Workflow | Scenarios | Result |
|---|---|---|
| `analyze` null | 5 layouts × 3 prevalences, plus 199 permutations | 3.85%–6.2%; every Bonferroni lower bound below 5% |
| `analyze` small foci | 5 layouts | power 95.2%–100%; lower bounds ≥ 0.93 |
| `analyze` one large focus (descriptive) | 5 layouts | 0.6% on the 16 × 16 square; 100% on the other four |
| `multimodal` null | 2 layouts × 2 prevalences × 4 null models | 1.2%–4.15%; all below 5% |
| `multimodal` immune association | 4 null models | power 500/500 under each |

Three findings came out of building this experiment, across earlier runs 01 (shared seed,
stopped after the small lattices), 02 and 03 (smoke-suite spectrum).

- **A shared permutation seed measures one reference set, not the test.** Unstratified null
  labels depend only on the cell count, the marked count and the seed. The earlier control
  reused one seed for every replicate, so all replicates shared one reference set, and its
  observed rate was the size of that set. Run 01 of this experiment, with the shared seed,
  gave 0.6%–11.85% across lattice layouts; with a seed per replicate the same scenarios
  gave 4.5%–5.8% (run 02). The same applies to a user who keeps the default seed: patterns
  with equal cell and marked counts reuse one reference set, whose size differs from alpha
  by Monte Carlo error with standard deviation about `sqrt(alpha (1 - alpha) / (B + 1))`.
  That is about 0.007 at the default 999 permutations and 0.034 at 39.
- **The quick-suite spectrum is not the shipped statistic.** Runs 02 and 03 reused the
  smoke-suite configuration, whose 5-shell spectrum on a 12 × 12 window never detected
  clustered labels (0/500). The shipped 64-shell spectrum detected small foci on the same
  window 62% of the time in a 300-replicate diagnostic. The calibration therefore uses the
  shipped configuration.
- **One large clone can be invisible to the primary endpoint.** The low-k endpoint uses
  wavelengths up to 0.33 of the window diagonal. One focus holding a quarter of the cells
  of a 16 × 16 window, about nine spacings across, was detected in 0.6% of replicates, while
  smaller foci in the same window were detected 95% of the time. A non-significant primary
  endpoint therefore does not rule out one large clone; inspect the marked-cell map.

Multimodal null rates lie below 5%, so its tests are conservative; small graphs give
discrete null distributions.

**Scope.** All of this uses synthetic layouts of up to 1,600 cells and fewer permutations
than the default. Stratified permutation, other alternatives and real tissue are not
covered. The multimodal runs use the smoke-suite parameters (99 permutations, 12 µm
radius). This evidence covers size and power only. It does not promote `analyze` or
`multimodal` beyond Research, because Experimental also requires agreement with an
independent implementation and a public real-data demonstration.

Run the quick suite with:

```text
marklab smoke --suite multimodal --replicates 25 --out smoke-multimodal
marklab smoke --suite synthetic --replicates 100 --out smoke-marked
```

Run the scheduled calibration into a new results directory with:

```text
MARKLAB_SHUFFLE_CALIBRATION_OUTPUT=<new-output-dir> \
  cargo +1.99.0 test --release --locked --package marklab --all-features --lib \
  shuffle_calibration -- --ignored --nocapture --test-threads=1
```

Regular CI on the main branch runs only a ten-replicate smoke test from the command
line. It does not check calibration.

## Published-data reference checks

The [pathology-study reference comparison](pathology-studies.md#reference-comparisons-on-published-cells)
runs the production L-minus-r workflow on published rabbit and cat retinal cell recordings.
It checks the border-corrected K estimator against spatstat with matched event intensities,
and checks the Gaussian intensity approximation against an independent analytic rectangle
calculation. A separate check executes GET against the existing tied-curve envelope oracle.
These numerical comparisons do not establish patient-group calibration or clinical validity.

The [Schürch CODEX](validation-schurch.md) and [Jackson IMC](validation-jackson.md)
comparisons use 35 and 100 real patients, respectively, with published group labels.
Independent SciPy/NumPy calculations agree with patient-level Max-T effects, Welch
statistics and Monte Carlo p-values within fixed tolerances. Schürch also checks the
CSV-to-multiplex Moran/Geary workflow. These are observational cohort examples and
numerical agreement checks, not estimates of type-I error or clinical performance.

## Limitations

- The quick suite is small on purpose and does not estimate clinical sensitivity or
  specificity.
- Scheduled calibration covers one tissue shape with uniform density, fixed label
  prevalences, one multimodal null model and one family of seeds.
- The pooled-bin before/after p-value is a rough check on curve bins, not a spatial or
  per-cell randomization test.
- Passing synthetic controls says nothing about the cell segmentation, classification,
  staining, sampling or landmark selection in a real laboratory workflow.
