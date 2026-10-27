# Variograms of tissue measurements

A semivariogram describes how dissimilar measurements become as the distance between
cells increases. For a vector of measurements `z`, the empirical value in a distance bin
is half the mean squared Euclidean difference between the vectors of all eligible
unordered cell pairs:

```text
gamma(bin) = sum_pairs ||z_i - z_j||² / (2 * number_of_pairs).
```

Marklab already supplies scalar, vector and projected-embedding variograms. The
unweighted vector calculation is the sum of the componentwise Matheron empirical
semivariograms. Its result is unchanged by an orthogonal rotation of the feature
coordinates, but changes under feature rescaling. Explicit pair weights replace the
pair count with the sum of weights in the denominator. Empty bins remain unavailable.

This is useful for describing whether marker expression or morphology varies mainly
between nearby cells or across larger tissue regions. A rising curve can indicate more
similar measurements among nearby cells. A flat curve can also arise from measurement
noise, mixed cell populations, or inadequate spatial resolution. A low curve can be
caused by low variance everywhere; it is not by itself evidence of spatial association.

## Model output in NPY format

The input can come from any model. An existing CSV may contain `object_id,x_um,y_um`
followed by feature columns. Alternatively, supply a row-major little-endian float32
NPY matrix and a CSV containing exactly `object_id,x_um,y_um`, with one coordinate row
for each matrix row in the same order:

```bash
marklab bayes vector-semivariogram \
  --input cell_coordinates.csv --vectors embeddings.npy \
  --bins distance_bins.csv --maximum-pair-visits 1000000 --out variogram.json
```

`distance_bins.csv` has `bin_id,lower_um,upper_um` columns. Feature names for NPY are
`embedding_0` through `embedding_(d-1)` in matrix column order. Coordinates must already
be in micrometres; there is no CellViT calibration, model-name, checkpoint or morphology
requirement on this descriptive analysis route. The statistic accepts 2–4,096
components; the underlying NPY v1/v2 reader supports widths up to 65,536.
There is a 128 MiB limit for the separate NPY file. CSV inputs retain their
16 MiB limit. A 256 MiB retained-data preflight accounts for the encoded NPY,
decoded float64 vectors, spatial rows and feature names before allocating the
decoded spatial input. Nonfinite vectors and mismatched row counts are errors.
The semivariogram is no longer restricted to 10,000 objects. Its result count must
fit in a `u32`, and the calculation still visits every unordered pair, bounded by
the declared `--maximum-pair-visits`. Accepting a larger matrix does not remove
that work limit.

The separate-NPY route writes `marklab.vector_semivariogram` version 2, binding the
coordinate CSV in `input_sha256` and the NPY in `vectors_sha256`. The digest is reused
from the NPY reader. CSV-only runs retain their version-1 result and numerical behavior.

The reader accepts any width in its bounded range; it does not identify or whitelist
models by dimension. Regression cases include 192, 384, 512, 768, 1,024, 1,152, 1,280,
1,536, 2,560 and 4,096 components, as well as nonstandard and boundary widths.
Examples of upstream output widths are:

| Output | Components | Producer documentation |
| --- | ---: | --- |
| ViT-Tiny | 192 | [timm model definitions](https://github.com/huggingface/pytorch-image-models/blob/main/timm/models/vision_transformer.py) |
| DINOv2 small/base/large/giant | 384 / 768 / 1,024 / 1,536 | [DINOv2 model definitions](https://github.com/facebookresearch/dinov2/blob/main/dinov2/models/vision_transformer.py) |
| CONCH contrastive projection | 512 | [CONCH configuration](https://github.com/mahmoodlab/CONCH/blob/main/conch/open_clip_custom/model_configs/conch_ViT-B-16.json) |
| UNI2-h | 1,536 | [UNI2-h model card](https://huggingface.co/MahmoodLab/UNI2-h) |
| Virchow2 class token / class token concatenated with mean patch tokens | 1,280 / 2,560 | [Virchow2 extraction example](https://huggingface.co/paige-ai/Virchow2) |
| DINOv3 SO400M / 7B definitions | 1,152 / 4,096 | [DINOv3 model definitions](https://github.com/facebookresearch/dinov3/blob/main/dinov3/models/vision_transformer.py) |

Import one vector per spatial object. Token selection, pooling and concatenation happen
upstream; use the actual exported width, which can differ from the encoder's hidden
dimension. Convert float16/bfloat16 exports to float32 before writing this NPY profile.
These declarations establish file compatibility, not validation of an encoder's biological
features. CLI checks use 3- and 2,560-component inputs with known semivariances and
row-count rejection.

## Public numerical comparison

The observed, unweighted two-feature vector workflow is **Experimental**. Existing
three-point known-answer tests cover pair weights and rotation invariance. The
[public comparison script](../examples/public-cohorts/compare_variograms.py) adds
SciKit-GStat 1.0.23 agreement on all 1,107 non-dirt cells in Schürch CODEX field A-reg001,
using the original compensated CD3 and CD20 measurements and the published
0.377442 µm/pixel calibration. The source is [CC BY 4.0](https://data.mendeley.com/datasets/mpjzbtfgfr/1).

The comparison fixes the distance edges at 0, 10, 25, 50 and 100 µm, uses unit pair
weights, and compares the native vector curve with the sum of independently calculated
SciKit-GStat scalar curves. Acceptance requires exact bin counts and scaled error
`abs(native-reference)/max(1,abs(reference)) <= 1e-12`. The executed run gave:

| Distance bin (µm) | Unordered pairs | Joint CD3/CD20 semivariance (AU²) |
|---|---:|---:|
| 0–10 | 903 | 62009.72766769836 |
| 10–25 | 4930 | 66314.58045840444 |
| 25–50 | 15585 | 65853.50312758257 |
| 50–100 | 49544 | 69103.99324548883 |

The maximum scaled error was `3.977532882391061e-15`. NumPy 2.5.3 and SciPy 1.18.1
were used. The initial run attempted a one-feature
input and was correctly rejected by the existing vector profile, which requires at
least two features. The final run compares the prespecified joint CD3/CD20 vector;
no numerical tolerance changed.

Marklab closes the last distance bin; SciKit-GStat uses upper-open bins. No pair is
exactly 100 µm apart in this field, which the script checks before comparison. The
script also preserves SciKit-GStat's default 10-bin curves, whose data-dependent
distance range differs from the prespecified physical scales. Those default curves are
descriptive comparisons, not equality targets. Neither calculation fits a model.

```bash
target/spatial-reference-venv/bin/python examples/public-cohorts/compare_variograms.py \
  /absolute/existing/path/schurch-source \
  /absolute/new/path/variogram-comparison
```

The external reference environment requires `scikit-gstat==1.0.23`. Download the
preserved source with [schurch.py](../examples/public-cohorts/schurch.py) as described
in [the cohort example](validation-schurch.md). The derived CSV uses the existing
`embedding_*` numerical-vector column names and records their explicit CD3/CD20
mapping; these are measured assay features, not claimed image-model embeddings.
The native command is currently named `bayes vector-semivariogram`, but this
particular calculation is descriptive and does not perform Bayesian inference.

## Assumptions and failure modes

- Distances require calibrated coordinates in one physical frame. Binning, window
  truncation, cell density and segmentation errors change which pairs contribute.
- The measurements must be comparable. Squaring raw marker differences emphasizes
  large-range channels and outliers. Choose transformations and scaling before analysis;
  this example uses raw compensated values only to check the stated estimator.
- A pooled curve combines within-type and between-type differences. It cannot separate
  cell composition from within-cell-type spatial variation without a declared design.
- Empirical curves can be computed without stationarity. Interpreting one as a spatial
  covariance model or fitting a range for prediction needs additional assumptions about
  stationarity, anisotropy and the mean; these are not verified here.
- The pair count is not an independent sample size. This public field is not a cohort
  test, a validation of the scalar permutation test, or evidence for kriging accuracy.

Projected, weighted and inferential variants retain their Research tier until their
own public demonstrations and reference checks are supplied. Kriging and fitted
variogram-model selection are outside this workflow.

Projected embedding variograms and embedding kernel mark correlations pool only
pairs within the same declared `biological_unit`, then summarize them by split.
Projected permutations also stay within each biological-unit/permutation-stratum
group, even when different units reuse a stratum name. Coordinates within a unit
must share one physical frame. For multiple unregistered slides from one patient,
run the spatial analysis separately for each slide; a patient identity alone does
not establish a shared coordinate frame. PCA centering and kernel scale fitting
still use the complete training split.

The estimator and binning conventions are described in the official
[SciKit-GStat variography guide](https://mmaelicke.github.io/scikit-gstat/userguide/variogram.html)
and [Variogram reference](https://mmaelicke.github.io/scikit-gstat/reference/variogram.html).
