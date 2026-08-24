# Part V — High-dimensional embedding science

## 23. Vector variograms and spatial covariance

### 23.1 Omnibus vector semivariogram — **ESTABLISHED extension**

```text
FUNCTION VectorSemivariogram(embedding_table, spatial_plan, distance_bins, weights = NONE):
    ValidateEmbeddingTable(embedding_table)
    REQUIRE embeddings and coordinates share stable object IDs
    sums = zeros(n_bins)
    pair_weights = zeros(n_bins)

    STREAM each eligible unordered pair (i,j,bin) from spatial_plan:
        delta = embedding_i - embedding_j
        sq_distance = StableDot(delta, delta)
        w = PairWeight(i,j,weights)
        sums[bin] += 0.5 * w * sq_distance
        pair_weights[bin] += w

    FOR bin:
        gamma[bin] = sums[bin] / pair_weights[bin] if positive else unavailable
    RETURN curve with pair counts, physical bins, and inference eligibility
```

This statistic is invariant to orthogonal rotation of the embedding coordinates.

### 23.2 Component or projected variogram — **ADVANCED**

```text
FUNCTION ProjectedEmbeddingVariograms(embeddings, projection_model, split_plan):
    fit centering/whitening/PCA/projection on training biological units only
    apply frozen projection to validation/test units
    compute scalar variogram per prespecified component
    control component × scale multiplicity using max-T or global hierarchy
    RETURN projection artifact and component variograms
```

### 23.3 Distance-binned embedding covariance matrix — **ADVANCED**

```text
FUNCTION EmbeddingCrossCovarianceByDistance(embeddings, spatial_plan, bins):
    global_mean = StableMeanRows(embeddings)
    FOR each bin:
        C_bin = zero matrix[d,d]
        count = 0
        STREAM pairs (i,j) in bin:
            C_bin += outer(embedding_i - mean, embedding_j - mean)
            count += 1
        C_bin = symmetrize(C_bin / count) for undirected summaries
        output trace(C_bin), Frobenius norm, leading eigenvalues, or full artifact
    RETURN low-dimensional invariant summaries in JSON and full matrices as artifact
```

### 23.4 Cross-covariance between two modalities — **ADVANCED**

```text
FUNCTION CrossModalCovarianceByDistance(A_embeddings, B_embeddings, pair_plan, bins):
    mean_A, mean_B = global or compartment-stratified means by declared policy
    FOR each A-B pair (i,j,bin):
        C_bin += outer(A_i - mean_A, B_j - mean_B)
    normalize by pair weights
    infer with source-section/compartment-stratified random labeling or patient-level comparison
    RETURN cross-covariance matrices and invariant norms
```

## 24. Kernel mark correlation

### 24.1 Kernel similarity curve — **ADVANCED**

```text
FUNCTION KernelMarkCorrelation(embeddings, spatial_plan, bins, kernel):
    REQUIRE kernel is positive semidefinite or explicitly labeled similarity only
    global_reference = mean_{i != j sampled/complete} kernel(z_i, z_j)

    FOR each distance bin b:
        numerator = weighted mean kernel(z_i, z_j) over eligible spatial pairs in b
        IF global_reference > tolerance:
            normalized[b] = numerator / global_reference
        ELSE:
            normalized[b] = unavailable(zero_global_kernel_reference)

    RETURN raw and normalized curves with kernel provenance
```

### 24.2 Random-label global-envelope test

```text
FUNCTION TestEmbeddingSpatialDependence(embeddings, coordinates, strata, B, seed, curve_fn):
    observed = curve_fn(embeddings)
    FOR b:
        permuted_rows = permute complete embedding vectors within declared strata
        null_curve[b] = curve_fn(permuted_rows)
    RETURN ERL global envelope
```

Never permute coordinates independently across embedding dimensions or permute each dimension separately.

### 24.3 Kernel choice

```text
FUNCTION BuildEmbeddingKernel(training_embeddings, kernel_spec):
    center/normalize using training set only
    SWITCH kernel_spec:
        LINEAR: k(a,b) = dot(a,b)
        COSINE: k(a,b) = dot(a,b)/(||a||||b||)
        RBF: bandwidth selected inside training folds
        LAPLACIAN: scale selected inside training folds
        LEARNED: metric/kernel learned with nested patient-held-out validation
    freeze kernel parameters and preprocessing artifact
    RETURN kernel
```

## 25. Graph smoothness and graph-frequency summaries

### 25.1 Dirichlet energy for scalar or vector signals — **ESTABLISHED**

```text
FUNCTION GraphDirichletEnergy(graph, signal_matrix_X, normalization):
    W = symmetric nonnegative graph weight matrix
    D = diagonal(row sums W)
    L = D - W or normalized Laplacian by declared policy

    numerator = trace(transpose(X) * L * X)
    SWITCH normalization:
        NONE: energy = numerator
        SIGNAL: energy = numerator / trace(transpose(X_centered) * X_centered)
        EDGE_WEIGHT: energy = numerator / sum(W)
    RETURN energy, graph digest, Laplacian convention
```

### 25.2 Random-label smoothness test

```text
FUNCTION GraphSmoothnessPermutationTest(graph, embeddings, strata, B, seed):
    observed = GraphDirichletEnergy(graph, embeddings, SIGNAL)
    FOR b:
        Z_b = permute embedding rows within strata
        null[b] = GraphDirichletEnergy(graph, Z_b, SIGNAL)
    # spatial smoothness means unusually low energy
    p_low = plus-one lower-tail p-value
    RETURN result
```

### 25.3 Local graph smoothness map — **EXPERIMENTAL/descriptive**

```text
FUNCTION LocalEmbeddingRoughness(graph, embeddings):
    FOR node i:
        local_i = sum_j W_ij * ||z_i - z_j||^2 / max(sum_j W_ij, epsilon)
    RETURN cell-level artifact
```

Local maps require multiplicity control before inferential hotspot labels.

## 26. Cell–patch–region complementarity

### 26.1 Explicit linkage validation

```text
STRUCT CellPatchLink:
    cell_id
    patch_id
    physical_scale_um
    relation ∈ {contains_cell_center, overlaps_nucleus, interpolated_context, nearest_patch}
    weight
    overlap_group_id

FUNCTION ValidateCellPatchLinks(cell_table, patch_table, links):
    REQUIRE all IDs resolve
    REQUIRE physical scale > 0
    REQUIRE weights finite and nonnegative
    REQUIRE one shared patch vector is referenced, not duplicated per cell
    compute overlap dependency groups
    RETURN validated link artifact
```

### 26.2 Aggregate patch context for a cell — **ESTABLISHED engineering**

```text
FUNCTION CellPatchContext(cell_id, patch_embeddings, links, scale):
    linked = links for cell_id and scale
    REQUIRE linked nonempty or return typed missing modality
    weights = normalize declared link weights
    context = weighted mean of referenced patch embedding rows using f64 accumulation
    RETURN context vector and effective number of independent patches
```

### 26.3 Incremental complementarity test — **ESTABLISHED predictive design**

```text
FUNCTION TestCellPatchComplementarity(dataset, target, nested_split_plan, model_family, metric):
    model_sets = {
        M0: technical + clinical + compartment + acquisition covariates,
        M1: M0 + cell embedding,
        M2: M0 + patch embedding,
        M3: M0 + cell + patch,
        M4: M3 + neighboring-cell context,
        M5: M4 + independently measured IHC/molecular features
    }

    FOR each outer patient-held-out fold:
        training, test = fold
        FOR each model set M:
            tune preprocessing, feature selection, scales, regularization, and hyperparameters
            using inner patient-held-out folds only
            fit final M on outer training
            predict outer test
            store patient-level predictions and calibration data

    compare nested model increments using patient-level paired performance differences
    use hierarchical bootstrap or paired permutation over outer-test patients
    RETURN M0–M5 performance, incremental value, calibration, and uncertainty
```

### 26.4 Overlap-aware patch effective sample size

```text
FUNCTION PatchDependencyWeighting(patches, overlap_graph):
    components or correlation clusters = connected/weighted overlap groups
    estimate effective patch count from overlap/correlation structure
    use patient as inferential unit regardless
    use weights only for within-specimen aggregation and descriptive uncertainty
    RETURN weights and diagnostic
```

## 27. Multiscale kernels and fingerprints

### 27.1 Multiscale embedding kernel

```text
FUNCTION MultiscaleEmbeddingKernel(sample_a, sample_b, scales, base_kernel, scale_weights):
    total = 0
    FOR scale l:
        representation_a = sample_a.embedding_summary(scale_l)
        representation_b = sample_b.embedding_summary(scale_l)
        total += scale_weights[l] * base_kernel(representation_a, representation_b)
    RETURN total
```

Scale weights must be prespecified or learned within training folds. Report sensitivity to scale grid.

### 27.2 Versioned spatial fingerprint construction

```text
STRUCT SpatialFingerprintSpec:
    component definitions
    physical axes
    normalization
    uncertainty weighting
    missing-component policy
    training-fitted transforms
    version

FUNCTION BuildSpatialFingerprint(project_sample, spec):
    components = []
    FOR component definition:
        compute or load validated endpoint
        align to canonical physical axis
        transform using frozen training-derived transform if any
        attach uncertainty and availability mask
        append components
    vector_or_structured_object = concatenate without discarding component identities
    RETURN SpatialFingerprint(spec_version, components, digest)
```

### 27.3 Fingerprint distance

```text
FUNCTION FingerprintDistance(A, B, metric_spec):
    REQUIRE same fingerprint specification/version
    total = 0
    FOR component c:
        IF both available:
            d_c = component distance, e.g. weighted L2 curve distance, Wasserstein, kernel distance
            total += weight_c * d_c
        ELSE:
            apply declared missing-component policy; never silently treat missing as zero
    RETURN distance plus component contribution decomposition
```

## 28. Retrieval and cross-sample testing

### 28.1 Analogous-region retrieval — **ADVANCED predictive/descriptive**

```text
FUNCTION BuildRegionRetrievalIndex(training_regions, fingerprint_or_embedding_spec, metric):
    fit all normalization and metric parameters on training cohorts only
    representations = compute frozen representation per region
    index = exact or approximate nearest-neighbor index
    record approximation recall against exact search on validation subset
    RETURN immutable retrieval index artifact

FUNCTION RetrieveAnalogousRegions(query_region, index, k, filters):
    validate query domain and provenance
    candidates = search index under filters for site/patient leakage policy
    return ranked candidates, distances, component explanations, and OOD score
    do not call nearest region biologically identical
```

### 28.2 Cross-sample embedding distribution test

```text
FUNCTION CompareEmbeddingDistributionsByPatient(patient_embedding_sets, groups, kernel, design, B):
    FOR each patient:
        summarize cell-level distribution using mean embedding, covariance, kernel mean embedding,
        or prespecified set kernel
    run PatientLevelMMD or EnergyDistance on patient summaries/set kernels
    RETURN cohort-level test
```

### 28.3 Within-patient region compatibility

```text
FUNCTION RegionCompatibility(region_A, region_B, fingerprint_spec):
    distance = FingerprintDistance(A,B)
    uncertainty = propagate endpoint/bootstrap/posterior uncertainty within patient
    RETURN descriptive compatibility only unless replicated patient-level design supports inference
```

## 29. M0–M5 predictive workflow

### 29.1 Leakage-safe training pipeline

```text
FUNCTION RunM0M5PredictiveWorkflow(project, target, model_families, outer_k, inner_k, seed):
    split_plan = BuildNestedCohortSplits(project.hierarchy, target.design, outer_k, inner_k, seed)

    FOR outer fold:
        train_projects, test_projects = split by patient/site policy

        FOR model level M0..M5:
            pipeline = BuildFeaturePipeline(M, training only)
            # fit normalization, whitening, PCA, feature selection, scale selection, imputation
            inner_scores = []
            FOR hyperparameter candidate and inner fold:
                fit pipeline and predictor on inner training
                evaluate on inner validation at patient-level target
            select hyperparameters by predeclared rule
            refit on all outer training
            predict outer test
            store raw score, probability/distribution, uncertainty, OOD, abstention

    aggregate patient-held-out predictions only
    compute discrimination/regression accuracy, calibration, decision curves if clinically defined,
    subgroup/site/scanner/stain performance, and uncertainty coverage
    compare adjacent M-levels with paired patient-level uncertainty
    RETURN complete model card and prediction artifacts
```

### 29.2 Probability calibration

```text
FUNCTION CalibratePredictions(training_oof_scores, training_labels, calibration_method):
    REQUIRE scores are out-of-fold at patient level
    fit Platt/logistic, isotonic, beta calibration, or Bayesian calibration on training OOF only
    apply frozen calibrator to held-out test predictions
    report reliability curve, calibration-in-the-large, slope, Brier score, ECE with bin uncertainty
```

### 29.3 Conformal prediction when exchangeability is defensible — **ADVANCED**

```text
FUNCTION GroupedConformalPredictor(train_units, calibration_units, test_units, score_fn, alpha):
    REQUIRE patient-level exchangeability or a justified weighted/conditional variant
    fit model on train units
    nonconformity = score_fn on calibration patients
    quantile = finite-sample corrected empirical quantile
    prediction_set(test) = outcomes with score <= quantile
    report coverage by site/subgroup and warn under distribution shift
```

### 29.4 OOD and abstention

```text
FUNCTION OODScore(test_representation, training_distribution, method):
    method ∈ {Mahalanobis_shrinkage, density_ratio, ensemble_disagreement, conformal_score,
              latent_distance, domain_classifier}
    fit on training only
    return score and calibrated threshold from validation domains

FUNCTION ApplyAbstention(prediction, uncertainty, ood_score, policy):
    IF ood_score > threshold OR uncertainty > threshold:
        return ABSTAIN(reason)
    ELSE return prediction
```

## 30. Calibrated multimodal fusion

### 30.1 Late fusion baseline — **ESTABLISHED**

```text
FUNCTION LateFusion(modality_predictions, validation_data, target):
    REQUIRE each base predictor trained without test leakage
    meta_features = out-of-fold base predictions + availability indicators
    fit regularized or Bayesian meta-model on patient-level OOF predictions
    calibrate fused output
    evaluate missing-modality scenarios and modality ablations
    RETURN fused predictor and contribution diagnostics
```

### 30.2 Product/mixture-of-experts fusion — **ADVANCED**

```text
FUNCTION MixtureOfExpertsFusion(modality_features, availability, context):
    expert_m outputs predictive distribution p_m(y | x_m)
    gating network g_m(context, availability) yields simplex weights
    p(y|x) = sum_m g_m * p_m(y|x_m)
    train using nested patient-held-out folds
    regularize gating to avoid collapse and site shortcuts
    calibrate final distribution
    RETURN predictor with missing-modality and OOD diagnostics
```

### 30.3 Bayesian model averaging / stacking

```text
FUNCTION PredictiveStacking(model_posteriors, pointwise_or_grouped_predictive_densities):
    optimize nonnegative weights summing to one to maximize grouped LOO predictive score
    use patient/specimen held-out unit
    form mixture predictive distribution
    RETURN weights with uncertainty/sensitivity; do not interpret as posterior model probability
```

---

