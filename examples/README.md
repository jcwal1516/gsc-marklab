# Pipelines

Marklab's commands are named for what they do, not for the data they were first written for: a
classifier fitted to MALDI-TOF spectra fits image embeddings or lab values just as well. This page
chains them for common situations. Each recipe says when to use it, which commands to run in which
order, and what to read in the output. The method pages linked from each recipe define the
statistics.

| Situation | Commands |
|---|---|
| [Predict a binary label from any features](#predict-a-binary-label-from-any-features) | `classifier fit`, `classifier predict` |
| [MALDI-TOF spectra to antimicrobial susceptibility](#maldi-tof-spectra-to-antimicrobial-susceptibility) | `spectrum preprocess`, `spectrum feature-table`, `classifier fit`, `classifier predict` |
| [Which features differ between two groups](#which-features-differ-between-two-groups) | `cohort max-t` |
| [Whether two sites or batches differ](#whether-two-sites-or-batches-differ) | `cohort mmd`, `cohort energy` |
| [Whether a label clusters among similar rows](#whether-a-label-clusters-among-similar-rows) | `project graph-smoothness-permutation-test` |
| [Matching peaks between spectra](#matching-peaks-between-spectra) | `bayes unbalanced-sinkhorn` |

The `cohort` commands call their unit `patient_id`; put whatever your independent unit is in that
column, such as an isolate or a sample.

## Predict a binary label from any features

Use when each row (an isolate, specimen, patient, slide…) has numeric features and a yes/no label,
and you want to know how well the features predict the label and what a model predicts for new
rows. Method: [calibrated binary classifier](../docs/classifier.md).

1. **Make a feature table**: a Parquet file with a text `id` column, numeric feature columns and
   `marklab.feature_table` metadata naming the modality, unit and provenance. Write it with
   `marklab spectrum feature-table` for spectra, or from Python
   ([example](../docs/classifier.md#feature-tables)).
2. **Write a label table**: a CSV with an ID column matching `id` and a label column, optionally a
   subset column (fit one species, say) and a group column (rows of one patient stay in one fold).
3. **Fit**: name the positive and negative label values in the configuration
   ([example](maldi-ast-fit.json)).

   ```sh
   marklab classifier fit --features features.parquet --labels labels.csv \
     --config fit.json --model model.json --report report.json
   ```

   Read `evaluation.mean_fold_auroc`, `brier_score` and `reliability` for how well the procedure
   predicts held-out rows, and `evaluation.conformal` for its calls: `false_negative_rate`,
   `false_positive_rate`, `abstention_rate` and per-class coverage.
4. **Predict** new rows from a table made the same way (same modality, unit, provenance and
   columns; anything else is refused):

   ```sh
   marklab classifier predict --model model.json --features new.parquet --out predictions.json
   ```

   Each row gets `probability_positive`, two conformal p-values and a `call`: `positive`,
   `negative`, `uncertain` (both classes plausible: decide another way) or `neither` (unlike the
   training rows).

The report's evidence is cross-validation on the training rows. A model that does well there may
not transfer to another instrument, site or year: test it on rows from there before relying on it.

## MALDI-TOF spectra to antimicrobial susceptibility

The binary-label pipeline with spectra as the features and resistance as the positive class.
Methods: [spectrum preprocessing](../docs/spectrum-preprocessing.md) and
[calibrated binary classifier](../docs/classifier.md).

1. **Preprocess each spectrum** (square root, Savitzky–Golay smoothing, SNIP baseline, total ion
   current, trim to 2–20 kDa, 6,000 bins of 3 Da):

   ```sh
   mkdir -p spectra
   for file in raw/*.txt; do
     code=$(basename "$file" .txt)
     marklab spectrum preprocess --text "$file" \
       --config examples/maldi-preprocess.json --out "spectra/$code.json"
   done
   ```

   Use `--bruker <acquisition directory>` instead of `--text` for Bruker flex `fid`/`acqu` data.
   For spectra from instruments that sample m/z at different spacings, add `"aggregate": "area"`
   to `binning` ([bin values](../docs/spectrum-preprocessing.md#configuration)).
2. **Collect the binned spectra** into a feature table, one row per isolate:

   ```sh
   marklab spectrum feature-table --spectra spectra --modality maldi_tof --unit isolate \
     --out maldi.parquet
   ```
3. **Fit one species and antibiotic.** [`maldi-ast-fit.json`](maldi-ast-fit.json) models
   *E. coli* and ceftriaxone with R and I as positive and S as negative. Add
   `"group_column": "<patient or case column>"` when one patient contributes several isolates.

   ```sh
   marklab classifier fit --features maldi.parquet --labels labels.csv \
     --config examples/maldi-ast-fit.json --model ecoli-ceftriaxone.model.json \
     --report ecoli-ceftriaxone.report.json
   ```
4. **Predict new isolates**, preprocessed with the same configuration:

   ```sh
   marklab classifier predict --model ecoli-ceftriaxone.model.json \
     --features new-isolates.parquet --out predictions.json
   ```

Susceptibility terms in the report, with resistance as the positive class:

| Susceptibility term | Report field |
|---|---|
| Very major error rate (resistant called susceptible, per resistant isolate) | `false_negative_rate` |
| Major error rate (susceptible called resistant, per susceptible isolate) | `false_positive_rate` |
| Categorical agreement | `threshold_0_5.accuracy` |
| Uncertain call | confirm with conventional AST |

The model takes categories as given: it does not read MICs or apply breakpoints, and it is not a
clinical susceptibility test. Spectra from another instrument or laboratory can differ
systematically; check with the next two recipes before predicting them.

## Which features differ between two groups

Use to find which features (spectrum bins, markers, embedding dimensions) differ between two
groups, with family-wise error control. Write a long CSV with columns
`patient_id,group,endpoint,value`, one row per unit and feature:

```sh
marklab cohort max-t --input features-long.csv --group-a R --group-b S \
  --permutations 1999 --seed 1 --alpha 0.05 --step-down --out max-t.json
```

Each endpoint gets the group A minus group B effect, a studentized statistic and a step-down
adjusted p-value. Inputs may be up to 512 MiB, and units × endpoints × (permutations + 1) at most
10⁸. For more features than that allows, test them in chunks and multiply each chunk's adjusted
p-values by the number of chunks (Bonferroni across chunks), which keeps family-wise control over
all features. Few positive units give little power: many small differences can predict well while
no single feature survives the correction.

## Whether two sites or batches differ

Use before carrying a model to another site, instrument or year, or to check whether a correction
(such as recalibrating spectra) removes a site difference. Sample units from both sites, choose the
features, standardise each over the pooled sample, and write `patient_id,group,feature,value`:

```sh
marklab cohort mmd --input sites-long.csv --group-a A --group-b B --kernel rbf \
  --bandwidth <median pairwise distance> --estimator unbiased --permutations 199 --seed 1 \
  --out mmd.json
```

`mmd_squared` measures the difference and `p_value` tests it; `cohort energy` is the
energy-distance version. The kernel matrix allows units² × (permutations + 1) ≤ 10⁸, which is 707
units at 199 permutations, so subsample larger sites. Keep the features, standardisation and
bandwidth fixed when comparing MMD² before and after a correction.

## Whether a label clusters among similar rows

Use to ask whether rows with the label sit next to each other in feature space, for example
resistant isolates sharing a lineage. Build a nearest-neighbour graph of the units yourself (say,
each unit joined to its 10 nearest by standardised Euclidean distance) and write two CSVs:
`node_id,permutation_stratum,signal_0` with the 0/1 label, and `left_node_id,right_node_id,weight`.

```sh
marklab project graph-smoothness-permutation-test --project project --nodes nodes.csv \
  --edges edges.csv --laplacian combinatorial --permutations 9999 --seed 1 \
  --maximum-nodes <units> --maximum-edges <edges> --maximum-dimension 1 \
  --maximum-component-edge-visits <edges × 10000> --memory-budget-mib 256 --out smoothness.json
```

A small `p_low` means fewer label changes along edges than under permutation: the label clusters.
Clustering is what shared lineage produces, but shared label-associated features produce it too; the
test does not tell them apart.

## Matching peaks between spectra

Use to align one spectrum's peaks with a reference's, for example to estimate a calibration shift,
while letting peaks without a partner go unmatched. Write the two peak lists as
`source_id,mass` and `target_id,mass` (heights divided by their sum) and a full
`source_id,target_id,cost` table, such as the squared difference in ln(m/z) divided by a squared
tolerance:

```sh
marklab bayes unbalanced-sinkhorn --source peaks.csv --target reference.csv --cost cost.csv \
  --epsilon 0.05 --tau-source 1 --tau-target 1 --tolerance 1e-9 --maximum-iterations 10000 \
  --out plan.json
```

The `plan` gives the mass moved between each pair. A plan-weighted mean of the reference position
minus the source position estimates the shift; moving the peaks by it and solving again refines it.
Check `converged` before using the plan.
