# Learned multimodal kernel fusion

`marklab multimodal kernel-fusion` learns a convex combination of patient similarity
kernels for a declared continuous target. It accepts named feature blocks such as pooled
patch embeddings, pooled cell embeddings, class composition and spatial summaries. Each
patient has exactly one row containing every declared modality.

```sh
marklab multimodal kernel-fusion \
  --input examples/multimodal-kernel-fusion.json \
  --out out/multimodal-kernel-fusion.json
```

The example uses 24 synthetic patients with one informative and one alternating feature.
Replace its `target_name`, `target_unit`, modality and feature names, measurements and fold
assignments with a prespecified research design. `features` follows modality order, and
each nested vector follows that modality's `feature_names`. All values must be finite;
missing blocks are rejected. Repeated samples must be reduced to the declared patient row
before entry. The command does not extract images, align cells to patches, pool slides or
choose an endpoint.

For each inner training set, Marklab fits feature means and population standard deviations,
then builds either a linear kernel or an RBF kernel:

```text
linear: K_m(i,j) = mean_d z_m(i,d) z_m(j,d)
RBF:    K_m(i,j) = exp[-mean_d (z_m(i,d)-z_m(j,d))² / (2 bandwidth_m²)]
fusion: K = sum_m alpha_m K_m, alpha_m >= 0, sum_m alpha_m = 1
ridge:  (K_train + lambda I) a = y_train - mean(y_train)
predict: mean(y_train) + K_test,train a
```

Each kernel is centered using training means and divided by its training mean diagonal.
The same training centering and scaling are applied to held-out similarities. A block with
negligible centered training trace is explicitly listed in `inactive_modalities` and
contributes a zero kernel; alone, it predicts the training target mean. No target values
from an outer test fold enter scaling, weight selection or ridge selection.

Declare `outer_fold` and `inner_fold` on each patient, plus `outer_folds` and `inner_folds`.
For each outer fold, its training patients are split by their declared inner fold. Every
inner validation fold must be nonempty and leave at least two training patients. Inner
validation mean squared error selects weights and a value from `ridge_penalties`. The
selected model is refit on the full outer training set and predicts only that fold's
held-out patients. Feature selection, dimensionality reduction and any upstream supervised
processing must also respect those splits; this command cannot undo upstream leakage.

The weight search enumerates integer simplex allocations summing to `weight_denominator`,
divided by that denominator, and includes equal weights. This is a finite grid search,
not a continuous global optimization. Fixed equal-weight and each single-modality model
are evaluated on exactly the same folds, with their ridge penalties independently selected
inside the training folds. Ties use a deterministic ordering. Patient input order does not
change the result.

The output reports patient predictions, outer-fold weights, ridge penalties, inner RMSE,
inactive blocks, and pooled held-out RMSE and MAE for each model. These are point estimates
for the supplied folds. Weights are neither Bayesian posterior probabilities nor causal
importance; correlated kernels can exchange weight with little prediction change. A
synthetic success does not establish added value on tissue cohorts. Compare the reported
baselines and existing standardized ridge on the same patient design before claiming a
benefit. Classification, survival likelihoods, prediction intervals, automated bandwidth
selection and a deployable full-cohort model are outside this workflow.

Use `kernel: {"kind": "linear"}` or `kernel: {"kind": "rbf", "bandwidth": 1.0}` in
each modality. Declare patient, total feature, weight-candidate, work and memory ceilings in
`limits` as shown in the example. The command rejects inadmissible work before kernel
construction and reports conservative work and peak-memory estimates, not measured usage.
CLI input is limited to 16 MiB, and output publication requires a new path.
