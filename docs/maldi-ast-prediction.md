# Predicting susceptibility from MALDI-TOF spectra and other features

Three commands turn per-isolate (or per-specimen) features into a calibrated model of one
antibiotic's resistance:

- `marklab microbio feature-table` collects binned MALDI-TOF spectra into a **feature table**.
  Other modalities, such as pooled image embeddings, write the same Parquet format directly.
- `marklab microbio fit-ast` evaluates the fitting procedure by cross-validation and fits the final
  model: an ensemble of L2-penalised logistic regressions with calibrated probabilities and
  conformal calls.
- `marklab microbio predict-ast` applies a model to a feature table made the same way.

The model is not a clinical susceptibility test. It does not read MICs or apply breakpoints.

```sh
marklab microbio preprocess \
  --text raw/<code>.txt \
  --config examples/maldi-preprocess.json \
  --out spectra/<code>.json                 # once per isolate

marklab microbio feature-table --spectra spectra --out maldi.parquet

marklab microbio fit-ast \
  --features maldi.parquet \
  --labels labels.csv \
  --config examples/maldi-ast-fit.json \
  --model ecoli-ceftriaxone.model.json \
  --report ecoli-ceftriaxone.report.json

marklab microbio predict-ast \
  --model ecoli-ceftriaxone.model.json \
  --features new-isolates.parquet \
  --out predictions.json
```

All three need the `parquet` feature, which default builds include.

## Feature tables

A feature table is a Parquet file with:

- a first column `id` (text; `string`, `large_string` or `string_view`), unique and 1 to 256
  bytes;
- one or more feature columns (`double` or `float`), with unique names and no missing or
  non-finite values;
- file metadata `marklab.feature_table` holding JSON
  `{"version": 1, "modality": …, "unit": "isolate" | "specimen", "provenance": …}`.

`provenance` is any JSON value that determines the features, such as preprocessing steps and bin
layout, or an image encoder and pooling rule. `predict-ast` accepts only tables whose modality,
unit, provenance and column names equal those of the model's training table. A table may hold at
most 100,000,000 values and 4 GiB.

`feature-table` reads every `*.json` file in `--spectra`, each a `marklab microbio preprocess`
result with `binning`, and uses the file name without `.json` as the ID. All results must share
the preprocessing steps and bins, which become the provenance; the columns are `bin_0`, `bin_1`,
… in m/z order and the modality is `maldi_tof`.

From Python, with pyarrow:

```python
import json, pyarrow as pa, pyarrow.parquet as pq

table = pa.table({"id": specimen_ids, **{f"h_{j}": embeddings[:, j] for j in range(dim)}})
meta = {"version": 1, "modality": "gram_stain_h_optimus", "unit": "specimen",
        "provenance": {"encoder": "h-optimus-0", "pooling": "mean"}}
table = table.replace_schema_metadata(
    {**(table.schema.metadata or {}), "marklab.feature_table": json.dumps(meta)})
pq.write_table(table, "gram.parquet")
```

## Labels

`--labels` is a CSV file of at most 16 MiB with a header row. The configuration's `target` names
its columns:

| Field | Meaning |
|---|---|
| `id_column` | Column matching the feature table's `id`; values must be unique |
| `antibiotic_column` | Column holding the antibiotic's category |
| `resistant`, `susceptible` | Category values of each class, such as `["R", "I"]` and `["S"]`; disjoint and non-empty |
| `species_column`, `species` | Optional, together: model only rows whose species equals `species` |
| `group_column` | Optional unit kept within one fold, such as the patient; each row is its own group when absent |

Rows of other species and rows with an empty category are counted and skipped. Any other category
must be listed; an unlisted value is rejected with its line number. Every remaining row's ID must
be in the feature table; feature rows without a label are counted and not used.

## Model settings

| Field of `model` | Meaning |
|---|---|
| `c_values` | Candidate inverse penalty strengths `C`, strictly increasing, at most 32 |
| `class_weight` | `balanced` (row weight `n / (2 n_class)`) or `uniform` |
| `folds` | Outer folds that evaluate the procedure, 2 to 20 |
| `inner_folds` | Folds within each fit, 2 to 20; also the number of ensemble members |
| `seed` | Seed of the fold shuffles |
| `significance` | Conformal significance `ε`, strictly between 0 and 1 |

## Method

**One fit.** Groups are dealt whole to `inner_folds` folds: groups containing a resistant row are
shuffled with the seed (ChaCha20) and dealt round-robin, then the remaining groups continue the
deal. There must be at least as many groups of each kind as folds. Every candidate `C` is fitted on
each fold's complement and scored on the held-out fold; the first (smallest) `C` with the highest
mean held-out AUROC is kept, and its fold models are the ensemble members.

Each member is an L2-penalised logistic regression on standardised features. It minimises

`½‖β‖² + C Σᵢ sᵢ [log(1 + exp(mᵢ)) − yᵢ mᵢ]`, with margins `mᵢ = x̃ᵢ·β + b`,

where `x̃` are the features standardised with the training mean and population standard deviation
(a constant column is centred and receives a zero coefficient), `sᵢ` is the class weight, and the
intercept `b` is not penalised: scikit-learn's `StandardScaler` followed by
`LogisticRegression(C)`. With fewer training rows than features, Newton's method works in the span
of the training rows (a Householder QR factorisation); otherwise in the features. At most
`min(rows, features) = 4,096` and `rows × features = 100,000,000` values are allowed per fit.

**Calibration.** Platt scaling, `P(resistant | m) = σ(a m + b)`, is fitted by maximum likelihood to
all members' held-out margins with Platt's smoothed targets, `(N₊ + 1)/(N₊ + 2)` and `1/(N₋ + 2)`.
A row's probability of resistance is the mean of the members' calibrated probabilities.

**Conformal calls.** Each member keeps the held-out margins of the rows it did not see, by class.
For a new row `x`, write `mₖ(x)` for its margin under member `k` and `k(i)` for the member that
did not see held-out row `i`, whose margin is `mᵢ`. The class-conditional cross-conformal p-values
are

`p_R = (1 + #{resistant held-out rows i : mᵢ ≤ m_k(i)(x)}) / (N_R + 1)` and
`p_S = (1 + #{susceptible held-out rows i : mᵢ ≥ m_k(i)(x)}) / (N_S + 1)`,

so each held-out row is compared with the new row under the member that did not see it. A class is kept
when its p-value exceeds `ε`. The call is `resistant` or `susceptible` when one class is kept,
`uncertain` when both are (confirm with conventional AST), and `neither` when neither is (the row
is unlike the training rows of both classes). Because the p-values are conditional on the class,
the fraction of resistant isolates called susceptible (very major errors) is controlled separately
from the susceptible isolates. Barber et al. (*Ann Stat* 2021) bound the coverage of cross-fitted
sets below by about `1 − 2ε`; in practice it is close to `1 − ε`, and the evaluation reports the
observed coverage. Both bounds assume new rows resemble the training rows (same instrument,
laboratory and population).

**Evaluation.** The groups are first dealt to `folds` outer folds in the same way. For each, the
whole fit above runs on the outer training rows and predicts the outer test rows; the final model is
then fitted on every row. The report therefore describes the procedure that produced the model.
It gives per-fold and pooled AUROC and average precision, the Brier score, a ten-bin reliability
table, and two sets of AST error rates: calls at calibrated probability 0.5, and conformal calls
with their abstention rate. A very major error calls a resistant isolate susceptible and its rate
is per resistant isolate; a major error calls a susceptible isolate resistant, per susceptible
isolate. AUROC counts ties as one half; average precision is scikit-learn's
`average_precision_score`.

## Outputs

**Model** (`--model`, JSON, `format` `marklab.ast-model`, `version` 1): the feature schema
(modality, unit, provenance, columns), the `target` and `model` settings, the label counts, and the
ensemble: selected `C`, members (standardisation, coefficients, intercept), Platt scaling,
significance and the held-out margins that calibrate the p-values. It contains no IDs.

**Report** (`--report`): the configuration, feature schema, label counts (`rows`, `other_species`,
`missing_category`, `labelled`, `resistant`, `susceptible`, `groups`, `unlabelled_feature_rows`),
`evaluation` (`folds`, AUROC and average precision, `brier_score`, `reliability`, `threshold_0_5`,
`conformal`, and each labelled row's out-of-fold `probability_resistant`, p-values, `call`,
`fold` and `resistant`), and `final_ensemble` (selected `C`, held-out AUROC per `C`, members).

**Predictions** (`predict-ast --out`): the model's modality, unit, antibiotic, species and
significance, and for every row of the table its `id`, `probability_resistant`,
`p_value_resistant`, `p_value_susceptible` and `call`.

Output files must not already exist.

## Evidence

Unit tests compare fitted probabilities with scikit-learn 1.9.0 (`newton-cholesky`, tolerance
1e-14) within 1e-7, for more features than rows (solved in the row span) with balanced weights and a
constant column, and for more rows than features with uniform weights; scikit-learn's own two
solvers agree only within 1e-8. Platt scaling matches a 50-digit Newton solution within 1e-12
(scikit-learn's `_sigmoid_calibration` is 2e-8 away). AUROC and average precision with tied scores
equal scikit-learn's. Conformal p-values of rows beyond every training row are exactly `1` and
`1 / (N + 1)`; a test fails when the p-value direction is reversed. Cross-validation keeps groups
in one fold, covers each class at least `1 − 2ε` on synthetic data, and repeats exactly from its
seed. A saved ensemble predicts identically after a JSON round trip, and inconsistent model files
are rejected. A CLI test runs `preprocess`, `feature-table`, `fit-ast` and `predict-ast` end to
end and checks the rejections above. Feature tables written by pyarrow 21 (`float` columns),
pandas 2.3 and polars 1.33 (`large_string` IDs) gave identical predictions.

## Not yet supported

- MIC values and breakpoint interpretation; categories are taken as given.
- Linking specimens to isolates: the label table's IDs must be the feature table's IDs.
- Combining modalities, L1 penalties, and models other than logistic regression.
- More than one species–antibiotic pair per fit, and temporal or site-based splits.
