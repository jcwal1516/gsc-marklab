# Jackson breast-cancer cohort validation

This comparison tests Marklab's patient-level independent-groups Max-T workflow on a real
imaging mass cytometry cohort. It is an executed method check, not a reanalysis of every
spatial and survival result in the source publication.

## Public source

The input is the curated Basel `SingleCellExperiment` for Jackson, Fischer et al. (2020),
published by `imcdatasets` as [`sce_basel.rds`](https://zenodo.org/records/20621983) under
CC BY 4.0. The [`imcdatasets` reference
page](https://bodenmillergroup.github.io/imcdatasets/reference/JacksonFischer_2020_BreastCancer.html)
documents that the subset has one tumour image for each of 100 patients and includes the
published cell metaclusters and spatial metadata. The underlying study is Jackson et al.,
[*The single-cell pathology landscape of breast
cancer*](https://doi.org/10.1038/s41586-019-1876-x).

The reproducible downloader and comparison driver is
[`examples/public-cohorts/jackson.py`](../examples/public-cohorts/jackson.py). It bounds the
download at 300 MiB, checks the license and publisher-declared byte count, preserves the raw
RDS outside the checkout, and uses base R only to read its serialized `colData`. It does not
run code from the data record.

## Prespecified contrast

The following design and acceptance limits were fixed before examining group differences:

- unit of inference: patient, with exactly one image per patient;
- group A: clinically ER-positive tumours; group B: clinically ER-negative tumours;
- exclusions: missing ER status; an unexpected nonbinary status fails validation;
- endpoints: the fractions of all metacluster-annotated cells belonging to each of
  metaclusters 1 through 13, calculated separately for every patient;
- multiplicity: two-sided, single-step Max-T over the complete 13-endpoint family;
- Marklab run: 19,999 whole-patient label permutations, seed 20261010, family-wise
  alpha 0.05;
- independent oracle: NumPy implementation of the Welch statistic and single-step Max-T,
  19,999 permutations with a different seed, 20261011;
- required structure: 100 patients, one image per patient, at least 20 patients per group,
  exact endpoint names and order, and finite nonzero observed standard errors;
- numerical acceptance: maximum absolute effect error at most `1e-12`, maximum absolute
  studentized-statistic error at most `1e-10`, and each adjusted-p-value difference within
  the Monte Carlo limit described below (originally a fixed `0.03`).

The original analysis pipeline [labels metaclusters 1 through 13 as immune, endothelial,
and stromal phenotypes and 14 through 27 as tumour-cell
metaclusters](https://github.com/BodenmillerGroup/SCPathology_publication/blob/master/R/BaselTMA_pipeline.Rmd#L471-L500).
Only the non-tumour phenotypes were included here. This avoids building an ER-status test
from tumour phenotypes whose definitions themselves include hormone-receptor expression.
The denominator remains all cells with a nonmissing published metacluster, so every endpoint
is an absolute fraction of the patient's annotated cells rather than a renormalized fraction
within the non-tumour compartment.

## Executed result

The run used 285,851 metacluster-annotated cells from 100 patients and confirmed exactly one
image for each patient. There were 72 ER-positive and 28 ER-negative patients. Marklab wrote
`marklab.cohort_max_t` version 1 with a complete-family critical value of
3.036110369380994.

An explicit source-row audit found 285,851 raw cells and zero missing patient IDs, image
names, metaclusters or ER statuses. The extraction now writes those counts, rejects missing
patient/image identities, and records missing metaclusters rather than hiding denominator
exclusions. Re-extraction reproduced every patient endpoint row exactly.

| Endpoint | ER-positive minus ER-negative fraction | Welch statistic | Max-T adjusted p |
|---|---:|---:|---:|
| metacluster 1 | 0.002282 | 0.890 | 0.99970 |
| metacluster 2 | -0.003696 | -0.645 | 1.00000 |
| metacluster 3 | -0.016822 | -0.547 | 1.00000 |
| metacluster 4 | -0.025402 | -1.134 | 0.97355 |
| metacluster 5 | -0.005214 | -0.758 | 1.00000 |
| metacluster 6 | 0.000635 | 0.930 | 0.99940 |
| metacluster 7 | -0.010130 | -1.387 | 0.88170 |
| metacluster 8 | 0.003220 | 0.196 | 1.00000 |
| metacluster 9 | 0.005635 | 0.373 | 1.00000 |
| metacluster 10 | 0.016692 | 0.728 | 1.00000 |
| metacluster 11 | 0.004626 | 0.706 | 1.00000 |
| metacluster 12 | -0.000052 | -0.503 | 1.00000 |
| metacluster 13 | 0.000010 | 0.003 | 1.00000 |

No endpoint rejected the complete-family null at alpha 0.05. The independent oracle also
rejected none and produced a critical value of 3.0382720699329107. Agreement passed every
prespecified limit:

| Quantity | Maximum absolute difference | Acceptance limit |
|---|---:|---:|
| group mean effect | 8.33e-17 | 1e-12 |
| Welch statistic | 3.55e-15 | 1e-10 |
| Max-T adjusted p-value | 0.0026 | Monte Carlo limit; largest ratio 0.30 |

The adjusted p-values are Monte Carlo estimates from deliberately independent random streams;
their small difference is expected. Each pair of adjusted p-values comes from independent Monte Carlo streams, so the limit
is four standard errors of their difference, `4 * sqrt(p * (1 - p) * (1/(B1+1) + 1/(B2+1)))`
plus one permutation step per side, where `p` is the mean of the two estimates and both
`B` are 19,999. Near p = 0.035 this is about 0.0075. It replaced a fixed 0.03 limit after
a review on 10 October 2026, because 0.03 could not detect an error large enough to move
a result across alpha = 0.05. The largest observed difference is 0.30 of its limit. Effects and observed statistics agree to floating-point
precision because they are deterministic functions of the same patient matrix.

The paper reports that fibroblast, endothelial, and immune populations occurred at similar
densities across clinical breast-cancer subtypes, while tumour-cell phenotypes varied with
hormone receptors and other subtype markers. The absence of a corrected difference in this
non-tumour endpoint family is qualitatively compatible with that report. The paper did not test
this exact 13-endpoint ER contrast on the curated 100-patient subset, so this is not a numerical
replication of a published table.

## Reproduction and scope

The executed command was:

```bash
python3 examples/public-cohorts/jackson.py \
  --output <output-dir> \
  --marklab target/debug/marklab
```

The output directory contains the preserved source metadata, patient-image mapping, aggregated
cell counts, Marklab input, native result, independent-oracle result, and machine-readable
comparison summary. The run used R 4.5.2, Python 3.14, and NumPy 2.4.4. A rerun with the
Monte Carlo p-value limit (Python 3.12.9, NumPy 2.5.2) reused the first run's raw source read-only
and reproduced identical native and oracle values.

To rerun into a fresh directory while preserving and reusing the raw RDS, pass
`--source <previous-output-dir>` and a new `--output`
path. The source remains read-only and must have the publisher-declared byte count.

This check establishes real-cohort execution and independent numerical agreement for the
unblocked two-group Max-T workflow. It does not establish that ER status causes compositional
differences, adjust for grade or other clinical covariates, validate spatial neighborhood or
survival analyses, or represent the full 281-patient Basel cohort. The one-image-per-patient
curated subset also cannot quantify within-patient spatial heterogeneity.
