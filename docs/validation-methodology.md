# Validation methodology

Marklab checks its statistics in two ways.

**`marklab smoke`** runs the real analysis pipeline on synthetic data where the right
answer is known: patterns with no structure, which must not be flagged, and patterns with
planted structure, which must be. It is quick and catches a broken pipeline.

**Scheduled calibration** runs 1,000 synthetic datasets with no structure through the
same engines every week and checks that the share flagged as significant is no higher than the
stated false-positive rate. That is the evidence that p-values mean what they say. Neither
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
| Negative | unrelated MMR-abnormal territories | separate production territory clusters |
| Negative | immune cells independent of MMR territory | adjusted enrichment p-value |
| Negative | registration jitter without association | enrichment remains negative after fitted registration |
| Negative | matched pre/post organization | production descriptive-margin result |
| Positive | related MMR-abnormal territories | one density-connected production territory |
| Positive | immune-enriched MMR territory | adjusted enrichment p-value |
| Positive | cross-interaction enrichment | production global cross-curve p-value |
| Positive | changed pre/post organization | production descriptive-margin result |
| Registration | residual above configured maximum | production engine rejection |
| Registration | association below registration resolution | production graph edge-resolution flag |
| Edge | too few landmarks | production input-validation error |
| Edge | degenerate landmarks | production rigid-fit error |
| Edge | empty H&E or IHC section | production fused-cell summary |
| Edge | no abnormal cells | available empty production territory result |
| Edge | sparse graph | production graph has no edges |
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

## Continuous integration

Every pull request runs two test jobs. The Rust job runs the native tests. The backend job
installs Python 3.12 from the lock file, runs `marklab backend doctor`, runs the full
all-feature test suite two tests at a time, and runs the Python tests in `tests/python`
and `workers/python`. Python-backed tests are required in that job; they are never skipped
because an environment is missing.

## Scheduled calibration

The calibration workflow (`.github/workflows/calibration.yml`) runs weekly, or on demand,
outside pull-request CI. It runs 1,000 synthetic datasets with no real structure through
each engine. The single-section control places a fixed number of labels at random on fixed
cell positions. The multimodal control randomizes both the H&E cell classes and the IHC
MMR labels, at fixed counts, because its null hypothesis shuffles both. The test passes
when the upper end of the 95% Wilson interval for the false-positive rate is at most 5%.

In one recorded run, the multimodal control produced 24 false positives in
1,000 replicates (2.4%; 95% Wilson interval 1.62%–3.55%) with 99 permutations.
That is below 5% and passes the rule.
The marked control produced 33 false positives in 1,000 replicates (3.3%; 95%
Wilson interval 2.36%–4.60%) with 39 permutations. Both results reproduce exactly on the same machine and seed, but each covers one tissue
shape and one label prevalence, and only one multimodal null model. Broader calibration
is still to be done.

Run the quick suite with:

```text
marklab smoke --suite multimodal --replicates 25 --out smoke-multimodal
marklab smoke --suite synthetic --replicates 100 --out smoke-marked
```

Run the scheduled calibration contract with:

```text
cargo +1.99.0 test --locked --all-features negative_control_calibrates \
  -- --ignored --nocapture --test-threads=1
```

Regular CI on the main branch runs only a ten-replicate smoke test from the command
line. It does not check calibration.

## Limitations

- The quick suite is small on purpose and does not estimate clinical sensitivity or
  specificity.
- Scheduled calibration covers one tissue shape with uniform density, fixed label
  prevalences, one multimodal null model and one family of seeds.
- The pooled-bin before/after p-value is a rough check on curve bins, not a spatial or
  per-cell randomization test.
- Passing synthetic controls says nothing about the cell segmentation, classification,
  staining, sampling or landmark selection in a real laboratory workflow.
