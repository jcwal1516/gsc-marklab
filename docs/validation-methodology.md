# Validation Methodology

## Scope and terminology

`marklab smoke` is a deterministic production-pipeline smoke suite. It checks
that declared controls reach expected production states; it is not, by itself,
evidence of statistical calibration, clinical validity, biological causality,
or fitness for diagnosis.

Marked scenarios construct a `Pattern` and call `AnalysisEngine`. Multimodal
scenarios construct H&E cells, IHC cells, landmarks, metadata, and an
`AnalysisConfig`, then call `MultimodalEngine::analyze_run`. Pre/post controls
analyze both inputs and call the same comparison services used by production
commands. Generators create inputs only. They do not insert result flags,
construct expected result DTOs, or set a scenario to pass unconditionally.

## Reported denominators

Every scenario reports:

- replicates attempted, completed, and failed;
- exact failure reasons;
- the observed criterion rate and a two-sided 95% Wilson interval;
- endpoint-specific rates where applicable;
- the fixed smoke acceptance criterion;
- the base seed and permutation-seed policy;
- relevant configuration and the crate version.

A failed replicate is retained in `replicates_failed` and fails the scenario. It
is never dropped from the report or treated as a negative observation.

## Multimodal production scenarios

The quick suite covers these controls:

| Class | Scenario | Production evidence |
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

The pre/post margin is descriptive. A result within the margin is not called
statistical equivalence, and a non-significant pooled-bin diagnostic is not used
as equivalence evidence.

The changed pre/post control preserves cell coordinates and swaps the H&E
label organization. This changes the cross-label spatial arrangement while
keeping the physical bin axis and geometric availability comparable. A
geometry change that makes one timepoint's bin undefined is reported as an
unavailable comparison, not coerced to an observed zero.

## Marked production scenarios

Marked controls exercise random labeling, clustered and multi-focus marks,
anisotropy, dispersed marks, density/stain artifacts, internal-control dropout,
fragmented components, rare phenotypes, and pre/post metadata mismatch. Their
criteria use production spectra, residual territories, anisotropy, QC flags,
suppression status, or pre/post comparison flags. Internal-control dropout sets
the internal-control fraction; it does not repurpose the overall retained-mask
fraction. The pre/post mismatch flag is produced by the comparison service.

## Smoke acceptance

The quick suite uses deliberately strong deterministic controls. Every scenario
must complete without an unexpected error and meet its declared property in at
least 80% of replicates. The random-label multimodal negative control requires
at least 90%. These are smoke guards selected to catch gross behavioral
regressions; they are not nominal error-rate claims.

## Scheduled calibration

The weekly and manually dispatched `.github/workflows/calibration.yml` workflow
runs 1,000 full production replicates per engine outside pull-request CI. The marked
control uses fixed-count random marks at fixed positions. The multimodal control
independently randomizes both H&E class labels and IHC MMR marks at fixed counts
because the production source-section null permutes both label fields. The
acceptance rule requires the upper endpoint of the 95% Wilson interval for the
false-positive rate to be no greater than the nominal 0.05 threshold.

At commit-under-test during Phase 9, the control produced 24 false positives in
1,000 replicates (2.4%; 95% Wilson interval 1.62%–3.55%) with 99 permutations.
This measurement is conservative relative to 5% and passes the declared rule.
The marked control produced 33 false positives in 1,000 replicates (3.3%; 95%
Wilson interval 2.36%–4.60%) with 39 permutations. These measurements are
machine- and seed-reproducible but cover one geometry and prevalence each, and
one multimodal null model; broader formal calibration remains scheduled
validation work.

Run the quick suite with:

```text
marklab smoke --suite multimodal --replicates 25 --out smoke-multimodal
marklab smoke --suite synthetic --replicates 100 --out smoke-marked
```

Run the scheduled calibration contract with:

```text
cargo +1.96.0 test --locked --all-features negative_control_calibrates \
  -- --ignored --nocapture --test-threads=1
```

Ordinary main-branch CI runs only the ten-replicate CLI smoke artifact. It does
not make or gate on a formal calibration claim.

## Limitations

- The quick suite is deliberately small and does not estimate clinical
  sensitivity or specificity.
- The scheduled calibration currently covers a single fixed-density geometry,
  fixed label prevalences, the source-section null, and one seed family.
- The pooled-bin pre/post p-value remains an approximate curve-bin diagnostic;
  it is not a spatial or per-cell randomization test.
- Passing synthetic controls does not validate upstream segmentation,
  classification, staining, sampling, or registration landmark selection in a
  real laboratory workflow.
