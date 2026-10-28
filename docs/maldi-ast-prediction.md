# Predicting susceptibility from MALDI-TOF spectra

`marklab microbio cross-validate-ast` estimates how well binned MALDI-TOF spectra predict one
antibiotic's susceptibility category within one species. It fits L2-penalised logistic regression
in nested, grouped cross-validation and reports out-of-fold AUROC and average precision together
with each isolate's out-of-fold probability of resistance. It does not keep a model for new
isolates, read MICs or apply breakpoints, and its results are not a clinical susceptibility test.

```sh
marklab microbio preprocess \
  --text raw/<code>.txt \
  --config examples/maldi-preprocess.json \
  --out spectra/<code>.json            # once per isolate

marklab microbio cross-validate-ast \
  --labels labels.csv \
  --spectra spectra \
  --config examples/maldi-ast-cross-validation.json \
  --out ecoli-ceftriaxone.json
```

## Inputs

**Label table** (`--labels`): a CSV file of at most 16 MiB with a header row and one row per
isolate. The configuration names the columns holding the isolate ID, the species and the
antibiotic's category, and optionally a group such as the patient. Rows of other species, and
rows of the species with an empty category, are counted and skipped. Any other category must be
listed as resistant or susceptible in the configuration; an unlisted value is rejected with its
line number. Isolate IDs must be unique and consist of 1 to 128 letters, digits, `-`, `_` or `.`,
not starting with `.`, because they name the spectrum files.

**Spectra** (`--spectra`): a directory holding `<isolate ID>.json` for every isolate that has a
category, as written by `marklab microbio preprocess` with `binning`. All of them must have the
same bins. Each file may be at most 512 MiB.

**Configuration** (`--config`), with unknown fields rejected:

| Field | Meaning |
|---|---|
| `isolate_column`, `species_column`, `antibiotic_column` | Column names in the label table |
| `group_column` | Optional column of the unit kept within one fold, such as the patient; each isolate is its own group when absent |
| `species` | Species to model, matched exactly |
| `resistant`, `susceptible` | Category values of each class, such as `["R", "I"]` and `["S"]`; disjoint and non-empty |
| `model.c_values` | Candidate inverse penalty strengths `C`, strictly increasing, at most 32 |
| `model.class_weight` | `balanced` (row weight `n / (2 n_class)`) or `uniform` |
| `model.folds`, `model.inner_folds` | Outer and inner folds, 2 to 20 each |
| `model.seed` | Seed of the fold shuffles |

## Method

Resistant isolates are the positive class. Within each training set the features are
standardised with the training mean and population standard deviation; a bin with the same value
in every training spectrum is centred on that value and receives a zero coefficient. The fit
minimises

`½‖β‖² + C Σᵢ sᵢ [log(1 + exp(mᵢ)) − yᵢ mᵢ]`, with margins `mᵢ = x̃ᵢ·β + b`,

where `sᵢ` is the class weight and the intercept `b` is not penalised: scikit-learn's
`StandardScaler` followed by `LogisticRegression(C)` with its default L2 penalty. With fewer
training isolates than bins the minimiser lies in the span of the training spectra, so Newton's
method works in that space (a Householder QR factorisation of the standardised spectra) with a
dense Cholesky solve and a backtracking line search. At most `min(isolates, bins) = 4,096` and
`isolates × bins = 100,000,000` values are allowed per training set.

Groups are dealt to folds whole. Groups containing a resistant isolate are shuffled with the seed
(ChaCha20) and dealt round-robin, then the remaining groups continue the deal, so each fold gets a
near-equal share of both. There must be at least as many groups of each kind as folds, both overall
and within each outer training set for the inner folds. Within each outer training set, inner
cross-validation scores every `C` by its mean inner-fold AUROC; the first (smallest) `C` with the
highest mean is refitted on the whole outer training set and predicts the outer test fold.

AUROC counts tied scores as one half (the Mann–Whitney statistic). Average precision is
`Σ (Rₖ − Rₖ₋₁) Pₖ` over decreasing score thresholds, as in scikit-learn's
`average_precision_score`. The mean of the outer folds' values estimates performance on a test set
the size of one fold; the pooled values rank all out-of-fold probabilities together, although each
fold's probabilities come from a different model.

## Output

The result JSON contains:

- `config`: the configuration as run;
- `labels`: `rows` read, `other_species`, `missing_category`, `isolates` modelled, `resistant`,
  `susceptible` and `groups`;
- `features`: the bins' `min_mz`, `max_mz` and `bins`;
- `folds`: for each outer fold, `training_rows`, `test_rows`, `test_positives`,
  `inner_mean_auroc` for every candidate `C`, `selected_c`, `auroc` and `average_precision`;
- `mean_fold_auroc`, `mean_fold_average_precision`, `pooled_auroc`, `pooled_average_precision`;
- `predictions`: for each isolate in table order, `isolate`, `group`, `fold`, `resistant` and
  `probability_resistant`.

The output file must not already exist.

## Evidence

Unit tests compare fitted probabilities with scikit-learn 1.9.0 (`newton-cholesky`, tolerance
1e-14), within 1e-7, for one problem with more bins than isolates (solved in the span of the
spectra), balanced weights and a constant bin, and one with more isolates than features and
uniform weights; scikit-learn's own two solvers agree only within 1e-8. AUROC and average
precision with tied scores equal scikit-learn's exactly. Cross-validation keeps every group in one
fold and repeats exactly from its seed. A CLI test runs `preprocess` and `cross-validate-ast`
end to end and checks the rejections above.

## Not yet supported

- Fitting a final model and predicting new isolates; calibrated probabilities.
- MIC values and breakpoint interpretation; categories are taken as given.
- L1 penalties and models other than logistic regression.
- More than one species–antibiotic pair per run.
