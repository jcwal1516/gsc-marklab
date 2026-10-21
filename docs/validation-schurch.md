# Schürch colorectal CODEX cohort validation

This check runs independent-patient inference on the published CLR and DII groups and
compares Marklab with SciPy. It also exercises the complete CSV-to-multiplex-study path
with real marker measurements and physical coordinates. It is a method validation on a
public cohort, not a reproduction of every result in the original paper.

## Source and sampling

The original `CRC_clusters_neighborhoods_markers.csv` is available from the
[Schürch et al. Mendeley record](https://data.mendeley.com/datasets/mpjzbtfgfr/1) under
CC BY 4.0. The downloader in
[`examples/public-cohorts/schurch.py`](../examples/public-cohorts/schurch.py) checks the
record's license and declared byte count and preserves the CSV outside the checkout.
Patient-group annotations and acquisition metadata come from the openly licensed
[TCIA collection](https://www.cancerimagingarchive.net/collection/crc_ffpe-codex_cellneighs/).
Only deidentified patient ID, group, and the group-defining Graham–Appelman category are
read from the metadata table. The cell-level group agrees exactly with that table:
17 CLR and 18 DII patients.

The full source has 258,385 cells in 140 images, four cores per patient across tissue
arrays A and B. Region names repeat between arrays, so image identity includes both
array and region. The composition analysis pools the four cores of each patient and
uses the nine supplied final cellular-neighborhood labels. There are 4,566 cells without
a final neighborhood; these are excluded from that denominator and counted explicitly.
Marklab imports these labels; it does not reproduce the paper's neighborhood clustering.

The multiplex analysis uses every non-`dirt` cell from array A: 138,567 cells, 70 images,
and two images for each of the same 35 patients. The exclusions are 116,589 cells from
array B and 3,229 `dirt` rows. The selected markers are the supplied compensated CD3 and
CD20 intensities. Coordinates are multiplied by 0.377442 µm per pixel, as reported in the
[paper's imaging methods](https://pmc.ncbi.nlm.nih.gov/articles/PMC7479520/). The field is
1920 × 1440 pixels; the [public-data image example](https://pathml-dev.readthedocs.io/en/latest/examples/link_loading_images.html)
documents those dimensions, and all retained coordinates lie within them. The rectangular
window represents the acquired image field, not a tissue segmentation. No area-normalized
density or point-process claim is made from this rectangle.

## Design and support amendment

Before comparing group differences, the analysis fixed a 100 µm radius, the two markers,
equal-image means within patients, whole-patient permutations, 19,999 permutations, seed
20261010 and family-wise alpha 0.05. Composition uses one nine-endpoint Max-T family;
Moran/Geary uses a separate four-endpoint family. These are two planned analyses, not one
joint 13-endpoint family. The independent reference uses seed 20261011.

Two initial runs failed:

- Run 01: the combined
  70-image study exceeded the native 100-million edge-evaluation cap. Execution was
  partitioned into two 35-image studies, each containing every patient. Their patient
  vectors are averaged equally before one final four-endpoint Max-T test. This preserves
  the observations, patient weighting, radius and endpoint family.
- Run 02: the original
  strict binary policy declared fields `A-reg003` and `A-reg017` unavailable because they
  contain isolated observed cells. No field was dropped. The explicit
  `binary_symmetric_including_isolates` policy was added after this support failure and
  before the successful comparison. It retains every observed cell in the mean, variance,
  sample size and permutation population, with zero graph weights for isolates. The
  original strict policies remain unchanged. This support amendment is recorded in the
  machine-readable design; it was not a prespecified part of the first failed run.

The all-cell convention follows the standard Moran/Geary formulas and agrees with
[PySAL/esda under binary weights](https://pysal.org/esda/v2.9.0/generated/esda.Moran.html).
The [API documentation](public-api.md#spatial-autocorrelation-weights) gives the exact
four-cell oracle and explains differences from package defaults. A zero-edge graph remains
unavailable. No numerical tolerance was changed after either failure.

## Independent checks and results

SciPy `cKDTree.query_pairs` constructs an independent radius graph. Direct Moran/Geary
formulas use every observed cell and must agree within `1e-10` absolute error, with exact
edge counts and patient membership. SciPy `ttest_ind(equal_var=False)` supplies the Welch
statistic. `permutation_test` supplies independently shuffled joint endpoint vectors;
their maximum absolute statistic yields the single-step Max-T reference. SciPy's default
two-sided unadjusted p-values are retained separately, not described as equivalent to
family-adjusted p-values. Acceptance limits were fixed before the comparisons:

| Quantity | Maximum absolute error | Limit |
|---|---:|---:|
| All 280 slide-marker Moran/Geary values | 2.498e-15 | 1e-10 |
| Nine-neighborhood patient mean effects | 5.551e-17 | 1e-12 |
| Nine-neighborhood Welch statistics | 1.110e-15 | 1e-10 |
| Nine-neighborhood adjusted p-values | 0.0056 | Monte Carlo limit (below) |
| Four spatial patient mean effects | 2.220e-16 | 1e-12 |
| Four spatial Welch statistics | 8.882e-15 | 1e-10 |
| Four spatial adjusted p-values | 0.0016 | Monte Carlo limit (below) |

Each pair of adjusted p-values comes from independent Monte Carlo streams, so the limit
is four standard errors of their difference, `4 * sqrt(p * (1 - p) * (1/(B1+1) + 1/(B2+1)))`
plus one permutation step per side, where `p` is the mean of the two estimates and both
`B` are 19,999. Near p = 0.035 this is about 0.0075. It replaced a fixed 0.03 limit after
a review on 10 October 2026, because 0.03 could not detect an error large enough to move
a result across alpha = 0.05. The largest
observed difference relative to its limit is 0.55 (CN5: 0.03305 versus 0.03715). Deterministic
effects and statistics agree to floating-point precision. Both execution partitions retain
all 35 patients; they contain 71,974 and 66,593 cells. Their graphs contain 15,650,016 and
12,326,252 directed edges per marker, respectively, within each study's work limit.

The follicle neighborhood, CN5, has a CLR-minus-DII patient fraction difference of
0.09024315318837443, Welch statistic 2.941801518906726 and Max-T adjusted p=0.03305.
No other neighborhood rejects at 0.05. This is qualitatively consistent with the study's
CLR sampling definition and organized lymphoid structures; it is not an independent
discovery of a feature unrelated to the group definition. The exact pooling and Max-T
contrast here differ from the publication's analyses.

| Spatial endpoint | CLR minus DII | Welch statistic | Max-T adjusted p |
|---|---:|---:|---:|
| CD3 Moran I | 0.0301595071 | 1.701531 | 0.32820 |
| CD3 Geary C | 0.0108705996 | 0.367318 | 0.99285 |
| CD20 Moran I | 0.0515829532 | 2.564174 | 0.05125 |
| CD20 Geary C | -0.0188181359 | -0.365128 | 0.99295 |

None of the four spatial endpoints rejects at the planned 0.05 level. In particular,
the CD20 Moran result is not rounded into significance. The 100 µm autocorrelation
analysis is a downstream Marklab example, not the paper's ten-nearest-cell neighborhood
method. This check does not establish causal, survival, diagnostic or clinical utility,
fitted-null calibration, whole-slide feasibility, or robustness to alternative radii.

## Reproduction

The executed comparison used Python 3.12, NumPy 2.5.2 and SciPy 1.18.1 from the existing
locked client environment:

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 target/client-venv/bin/python \
  examples/public-cohorts/schurch.py \
  --source <source-dir> \
  --out <output-dir>
```

Run 04 applies the Monte Carlo p-value limit. Its effects, adjusted p-values and reference
values are identical to those of run 03, which used the former fixed limit. One native
Welch statistic (CD3 Geary) differs by one binary64 step, 5.6e-17, consistent with the
correctly rounded JSON number parsing adopted after run 03.

For a new raw download, first run the script with `--download` and a new external directory.
For another comparison, reuse the read-only source and choose a fresh `--out`. The output
contains the design, explicit denominators and exclusions, recipes, native study projects
and results, patient tables, default SciPy comparisons and the agreement summary.

The script also exercises a sparse AnnData H5AD derived from the deterministic first field,
`A-reg001`: 1,107 non-dirt cells and the two measured markers. AnnData 0.12.4 writes and
reads the file, then the Marklab adapter must reconstruct the entire slide record exactly,
including IDs, physical coordinates, marker values, patient/group and window. This passed.
The H5AD contains
observed biological data but is a derived interchange file, not a claimed upstream export.

The separate ignored Rust test
`io::parquet_tests::public_schurch_cells_round_trip_through_csv_parquet_and_geojson`
checks the same field in the standalone binary-pattern profile. It imports coordinates
with the same calibration and uses membership in the supplied B-cell category as the
binary mark (117 of 1,107 cells), then writes and reads Parquet. Coordinates, marks and
metadata agree exactly. Its GeoJSON is the acquired field and its admission flags are
set for this I/O check; they are not inferred tumor identity or laboratory QC results.
No marked-pattern inference is run.
