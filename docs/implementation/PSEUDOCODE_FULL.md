# Marklab Frontier Spatial Pathology — Research-Backed Algorithm Pseudocode Specification

**Status:** implementation blueprint, not executable code  
**Companion charter:** `marklab_frontier_spatial_pathology_master_plan.md`  
**Scope:** the algorithmically complex work after the classical K/L foundation  
**Units:** physical distances are in micrometres unless an input contract explicitly declares another unit

---

## 0. Purpose and interpretation

This document translates the frontier Marklab program into explicit, implementation-oriented pseudocode. It covers every method family requested after the classical point-process foundation:

1. cohort-valid spatial inference;
2. Bayesian spatial modeling;
3. Bayesian point processes;
4. high-dimensional embedding science;
5. registration, atlas mapping, and transport;
6. graph and higher-order tissue mathematics;
7. topology and mathematical morphology;
8. multimodal Bayesian models;
9. generative tissue modeling and simulation-based inference;
10. 3-D, longitudinal, and evolutionary models;
11. causal, interference, perturbational, and active-design research.

The algorithms are not all at the same maturity level. Each specification is labelled:

- **ESTABLISHED:** the statistical object and principal algorithm are well established;
- **ADVANCED:** established methodology with substantial implementation and diagnostic burden;
- **EXPERIMENTAL:** credible research method, but not suitable for an unqualified stable claim;
- **RESEARCH_ONLY:** exploratory method whose identifiability or validation is highly design-dependent.

The pseudocode is deliberately stricter than ordinary library sketches. Every inferential or fitted result must carry:

- the estimand or prediction target;
- the biological replication unit;
- the randomization, likelihood, prior, or generative assumptions;
- exact versus approximate execution mode;
- typed failure and unavailable states;
- seeds, backend versions, artifact digests, and coordinate frames;
- calibration and diagnostics;
- claim maturity.

---

# Part I — Shared algorithmic substrate

## 1. Pseudocode notation

```text
REQUIRE condition ELSE RETURN TypedFailure(...)
ASSERT_INTERNAL condition
FOR EACH x IN collection
PARALLEL_FOR ... WITH deterministic reduction order
STREAM ...                    # bounded-memory iterator
ARTIFACT<T>                   # immutable checksum-addressed large output
OPTION<T>                     # typed optional value; never numeric sentinel
RESULT<T, Error>
EXACT | APPROXIMATE | LEARNED
```

All sums involving floating-point observations use stable reduction and `f64` accumulation, even when source matrices are stored as `f32`.

## 2. Shared domain contracts

### 2.1 Identity and cohort hierarchy

```text
TYPE PatientId, SpecimenId, TimepointId, SlideId, CoreId, RegionId, CellId,
     PatchId, ModalityId, SiteId, BatchId = validated non-empty identifiers

STRUCT CohortNode:
    id
    node_type ∈ {site, patient, specimen, timepoint, slide, core, region, cell, patch}
    parent_id: OPTION<Id>
    metadata: typed key/value map

STRUCT CohortHierarchy:
    nodes: immutable table<CohortNode>
    parent_index
    children_index
    digest

FUNCTION ValidateCohortHierarchy(hierarchy):
    REQUIRE all IDs unique
    REQUIRE all declared parents exist
    REQUIRE no cycles
    REQUIRE every child type is legal beneath parent type
    REQUIRE every analysis unit maps to exactly one biological parent at each required level
    RETURN ValidatedHierarchy(hierarchy, digest)
```

### 2.2 Coordinate, window, and spatial objects

```text
ENUM Dimension = D2 | D3

STRUCT CoordinateFrame:
    id
    dimension
    units
    axis_order
    origin
    orientation
    parent_frame: OPTION<CoordinateFrameId>
    transform_to_parent: OPTION<TransformArtifactRef>

STRUCT ObservationWindow:
    dimension
    coordinate_frame
    exact_geometry_artifact
    area_or_volume
    boundary_index_artifact
    components
    holes
    digest

STRUCT SpatialObjectTable:
    stable_ids
    coordinates[f64]
    coordinate_frame
    object_type
    compartment_ids: OPTION[array]
    qc_flags
    digest

FUNCTION ValidateSpatialObjects(objects, window):
    REQUIRE coordinate frames are identical or a declared transform exists
    REQUIRE all coordinates finite
    REQUIRE every retained object lies inside or on the permitted window
    REQUIRE units are physical and compatible
    RETURN validated objects
```

### 2.3 Marks and embeddings

```text
ENUM MeasurementStatus =
    MEASURED |
    IMPORTED_PREDICTION |
    MORPHOLOGY_DERIVED_PREDICTION |
    DERIVED_SUMMARY

STRUCT ScalarMarkTable:
    object_ids
    values
    mark_type ∈ {binary, categorical, ordinal, continuous, probability, simplex}
    units: OPTION<Unit>
    measurement_status
    provenance
    missingness_mask
    digest

STRUCT EmbeddingTable:
    object_ids
    values: row-major matrix<f32>[n_objects, dimension]
    model_name
    model_version
    weights_sha256
    encoder
    layer
    pooling
    source_modality
    stain
    micrometres_per_pixel
    physical_context_um
    preprocessing_digest
    normalization
    confidence
    missingness_mask
    digest

FUNCTION ValidateEmbeddingTable(table, identity_table):
    REQUIRE row count equals object ID count
    REQUIRE all present values finite
    REQUIRE embedding dimension > 0
    REQUIRE object IDs unique and resolvable
    REQUIRE model/checkpoint/layer/pooling provenance present
    REQUIRE physical context and resolution present for image-derived embeddings
    RETURN ValidatedEmbeddingTable
```

### 2.4 Cohort design and exchangeability

```text
ENUM DesignKind =
    INDEPENDENT_GROUPS |
    PAIRED |
    REPEATED_MEASURES |
    MULTIREGION |
    MULTISITE |
    CLUSTER_RANDOMIZED |
    SPATIAL_INTERFERENCE

STRUCT CohortDesign:
    kind
    biological_unit                    # usually patient
    repeated_unit: OPTION<node_type>
    treatment_or_group_field
    pair_id_field: OPTION<field>
    time_field: OPTION<field>
    site_field: OPTION<field>
    block_fields
    nuisance_covariates
    permitted_randomization_unit
    estimand

STRUCT ExchangeabilityBlock:
    block_id
    unit_ids
    allowed_operations ∈ {permute_labels, sign_flip, circular_shift, fixed}

STRUCT ExchangeabilityPlan:
    blocks
    whole_plot_restrictions
    pair_restrictions
    site_restrictions
    digest

FUNCTION BuildExchangeabilityPlan(design, hierarchy):
    ValidateCohortHierarchy(hierarchy)
    SWITCH design.kind:
        CASE INDEPENDENT_GROUPS:
            group biological units within declared site/block strata
            allow group-label permutation only at biological-unit level
        CASE PAIRED:
            create one block per pair
            allow within-pair swap or paired sign flip
        CASE REPEATED_MEASURES:
            keep all repeated observations for a biological unit together
            permit subject-level residual/sign-flip scheme declared by model
        CASE MULTIREGION:
            regions remain nested within patient
            randomize at patient level unless estimand is explicitly within-patient
        CASE MULTISITE:
            preserve site composition or use site-level meta-analysis
        CASE CLUSTER_RANDOMIZED:
            randomize treatment at randomized cluster level only
        CASE SPATIAL_INTERFERENCE:
            use known assignment mechanism and exposure mapping; no generic permutation
    REQUIRE plan preserves design-implied joint distribution under null
    RETURN ExchangeabilityPlan
```

### 2.5 Execution, seeds, and artifacts

```text
ENUM ExecutionMode = EXACT | APPROXIMATE | LEARNED
ENUM MaturityTier = VALIDATED | ESTABLISHED | EXPERIMENTAL | RESEARCH_ONLY

FUNCTION DeriveSeed(base_seed, endpoint_namespace, replicate_index, chain_id = 0):
    payload = canonical_encode(base_seed, endpoint_namespace, replicate_index, chain_id)
    RETURN first_u64(SHA256(payload))

STRUCT AlgorithmRunMetadata:
    algorithm_id
    algorithm_version
    execution_mode
    maturity_tier
    backend_name
    backend_version
    environment_digest
    source_commit
    config_digest
    input_artifact_digests
    seeds
    wall_time
    peak_memory

STRUCT ArtifactRef:
    schema_id
    schema_version
    sha256
    byte_length
    media_type
    relative_uri
```

### 2.6 Typed states

```text
ENUM Availability<T>:
    AVAILABLE(value: T)
    DISABLED(reason)
    NOT_APPLICABLE(reason)
    INSUFFICIENT_DATA(reason)
    INVALID_INPUT(reason)
    UNSUPPORTED_DESIGN(reason)
    NUMERICAL_FAILURE(reason, diagnostics)
    NONCONVERGED(reason, diagnostics)
    EXTERNAL_ARTIFACT_MISSING(reason)
    APPROXIMATION_NOT_PERMITTED(reason)

RULE: never encode missing, undefined, failed, or unavailable output as zero, NaN, infinity,
      empty vector, or a silently absent key.
```

## 3. Shared numeric and statistical helpers

### 3.1 Stable vector and matrix summaries

```text
FUNCTION StableMean(values):
    REQUIRE every value finite
    sum = compensated_sum(values)
    RETURN sum / len(values)

FUNCTION StableCovarianceMatrix(X):
    REQUIRE X finite
    center = columnwise StableMean
    C = zero matrix<f64>[d,d]
    FOR EACH row x IN X:
        delta = x - center
        rank_one_accumulate(C, delta, delta) using blocked f64 arithmetic
    RETURN C / (n - 1)
```

### 3.2 Multiplicity policy

```text
STRUCT MultiplicityFamily:
    family_id
    hypotheses
    correction ∈ {ERL_GLOBAL_ENVELOPE, MAX_T, WESTFALL_YOUNG, BH_FDR, BONFERRONI,
                  HIERARCHICAL, NONE_DESCRIPTIVE}
    alpha

FUNCTION AdjustPValues(p_values, family):
    REQUIRE all p-values finite in [0,1]
    SWITCH family.correction:
        CASE BH_FDR: return BenjaminiHochberg(p_values)
        CASE BONFERRONI: return min(1, m * p_i)
        CASE WESTFALL_YOUNG: require joint permutation statistics; use max-T distribution
        CASE HIERARCHICAL: apply predeclared tree gatekeeping
        CASE NONE_DESCRIPTIVE: return no inferential adjusted values
        ELSE: error because curve methods use their own global procedure
```

### 3.3 Deterministic permutation runner

```text
FUNCTION RunRestrictedPermutations(observed_data, plan, B, seed, statistic_fn):
    REQUIRE B > 0
    observed = statistic_fn(observed_data)
    null_stats = preallocate(B, shape(observed))

    PARALLEL_FOR b = 0 .. B-1 WITH output stored at index b:
        rng = DeterministicRng(DeriveSeed(seed, "restricted_permutation", b))
        permuted = ApplyExchangeabilityOperation(observed_data, plan, rng)
        null_stats[b] = statistic_fn(permuted)

    REQUIRE all completed replicates retained; failures are not dropped
    RETURN {observed, null_stats, failed_replicates}
```

### 3.4 Functional global test and envelope

```text
FUNCTION ExtremeRankLengthGlobalEnvelope(observed_curve, null_curves, eligible, alpha):
    REQUIRE identical axes and finite eligible values
    curves = [observed_curve] + null_curves
    FOR EACH curve point j:
        ranks_j = average ranks across curves with ties
        two_sided_rank = min(rank, N + 1 - rank)
    FOR EACH curve i:
        rank_vector_i = sort(two_sided_rank_i over eligible points ascending)
    order curves lexicographically by rank_vector
    preserve ties using average depth ranks
    p_global = (1 + number of null curves at least as extreme as observed) / N
    critical_depth = alpha-derived depth threshold
    envelope = pointwise min/max of curves whose depth is inside retained set
    RETURN GlobalEnvelope(p_global, depth_observed, envelope, eligible)
```

### 3.5 Nested patient-held-out validation splitter

```text
FUNCTION BuildNestedCohortSplits(hierarchy, design, outer_k, inner_k, seed):
    units = unique biological units declared by design
    stratify using group/site/outcome bins only when feasible
    outer_splits = deterministic grouped split of units
    FOR EACH outer split:
        training_units, test_units = split
        REQUIRE no patient, specimen, slide, adjacent section, patch, or linked cell leaks
        inner_splits = grouped split within training_units only
        save split artifact with digest
    RETURN NestedSplitPlan
```

## 4. Shared Bayesian model and fit contracts

### 4.1 Model intermediate representation

```text
STRUCT PriorSpec:
    family
    parameters
    truncation_or_transform
    rationale
    sensitivity_grid

STRUCT ParameterSpec:
    name
    shape
    support
    prior
    interpretation

STRUCT LikelihoodSpec:
    family
    link
    observation_unit
    dispersion
    censoring_or_missingness

STRUCT LatentFieldSpec:
    field_type
    domain
    precision_or_covariance
    constraints
    approximation

STRUCT BayesianModelIR:
    model_id
    parameters
    likelihoods
    latent_fields
    deterministic_transforms
    hierarchy
    priors
    generated_quantities
    posterior_predictive_statistics
    identifiability_constraints
    backend_capabilities_required
    maturity_tier
```

### 4.2 Bayesian fit lifecycle

```text
FUNCTION FitBayesianModel(model_ir, data, inference_spec, seed):
    ValidateModelIR(model_ir, data)
    prior_check = RunPriorPredictiveChecks(model_ir, data.context, seed)
    REQUIRE prior_check does not imply physically impossible mass beyond declared tolerance

    backend = ResolveBackend(inference_spec, model_ir.backend_capabilities_required)
    compile_artifact = backend.compile(model_ir)

    fit = SWITCH inference_spec.method:
        HMC_OR_NUTS      -> RunHmcNuts(...)
        SMC              -> RunSequentialMonteCarlo(...)
        VARIATIONAL      -> RunVariationalInference(...)
        LAPLACE          -> RunLaplaceApproximation(...)
        INLA_STYLE       -> RunInlaStyleApproximation(...)
        EXCHANGE_MCMC    -> RunExchangeMcmc(...)
        SBI              -> RunSimulationBasedInference(...)

    diagnostics = DiagnosePosteriorFit(fit, model_ir, data)
    predictive = RunPosteriorPredictiveChecks(fit, model_ir, data)
    sensitivity = RunPriorSensitivity(model_ir, data, inference_spec)

    IF diagnostics indicates invalid or nonconverged:
        RETURN NONCONVERGED(fit artifact retained, diagnostics)

    RETURN PosteriorArtifact(fit, diagnostics, predictive, sensitivity, provenance)
```

### 4.3 Posterior diagnostic gate

```text
FUNCTION DiagnosePosteriorFit(fit, model_ir, data):
    diagnostics = {}
    IF fit has chains:
        diagnostics.rank_normalized_rhat = compute split rank-normalized R-hat
        diagnostics.bulk_ess = effective sample size for location
        diagnostics.tail_ess = effective sample size for tails
        diagnostics.mcse = Monte Carlo standard errors
        diagnostics.divergences = count divergent transitions
        diagnostics.treedepth_hits = count maximum-depth hits
        diagnostics.ebfmi = energy Bayesian fraction of missing information
    IF fit is variational:
        diagnostics.elbo_trace
        diagnostics.multi_start_stability
        diagnostics.importance_correction = optional PSIS correction
    IF fit is Laplace/INLA-style:
        diagnostics.mode_gradient_norm
        diagnostics.hessian_condition
        diagnostics.approximation_comparison_on_small_fixture
    diagnostics.posterior_finiteness
    diagnostics.constraint_residuals
    diagnostics.identifiability_checks
    RETURN diagnostics
```

### 4.4 Posterior predictive laboratory

```text
FUNCTION RunPosteriorPredictiveChecks(fit, model_ir, observed_data):
    stats = model_ir.posterior_predictive_statistics
    FOR s = 1 .. n_predictive_draws:
        theta = draw posterior sample deterministically by stored index
        replicated = simulate from model likelihood/generative process
        FOR EACH statistic T IN stats:
            store T(replicated)
    compare observed T to predictive distribution
    include spatial summaries such as density, K, g, mark correlation, graph degree,
    component sizes, interface distance, topology, and embedding covariance as appropriate
    RETURN predictive diagnostics with no binary "pass" simplification
```

---

# Part II — Cohort-valid spatial inference

## 5. Patient-level permutation and hierarchical bootstrap

### 5.1 Patient-level restricted permutation test — **ESTABLISHED**

```text
FUNCTION PatientLevelPermutationTest(
    patient_records,
    design,
    endpoint_fn,
    B,
    seed,
    alternative,
    studentize = true
):
    plan = BuildExchangeabilityPlan(design, patient_records.hierarchy)
    REQUIRE design.permitted_randomization_unit == patient or randomized cluster
    REQUIRE endpoint_fn returns one prespecified value or curve per biological unit

    patient_endpoints = endpoint_fn(patient_records)       # no cell-level replication
    observed = GroupContrast(patient_endpoints, design, studentize)

    null = []
    failures = []
    FOR b = 0 .. B-1:
        rng = DeterministicRng(DeriveSeed(seed, "patient_permutation", b))
        assignment_b = RestrictedShuffle(design.assignment, plan, rng)
        TRY:
            stat_b = GroupContrast(patient_endpoints with assignment_b, design, studentize)
            append null, stat_b
        CATCH error:
            append failures, {b, error}

    REQUIRE no failed replicate is silently removed
    p = InclusivePlusOnePermutationP(observed, null, alternative)
    RETURN PermutationResult(observed, p, B_attempted=B, B_completed=len(null), failures, plan)
```

**Implementation notes**

- For independent patient groups, permute group labels only within declared site/block strata.
- For randomized trials, use the known assignment mechanism rather than an ad hoc shuffle.
- Studentization is usually preferable when group variances differ.
- Curves must use a joint global statistic or global envelope; do not test each radius independently without multiplicity control.

### 5.2 Hierarchical bootstrap — **ESTABLISHED but design-sensitive**

```text
FUNCTION HierarchicalBootstrap(
    hierarchy,
    levels,                   # e.g. patient -> specimen -> slide -> region
    statistic_fn,
    B,
    seed,
    within_lowest_unit_policy
):
    ValidateCohortHierarchy(hierarchy)
    REQUIRE levels begin with the highest independent biological unit
    REQUIRE levels follow legal parent-child order

    observed = statistic_fn(all observed records)
    bootstrap_stats = preallocate(B)

    FOR b = 0 .. B-1:
        rng = DeterministicRng(DeriveSeed(seed, "hierarchical_bootstrap", b))
        sampled_records = []

        sampled_top_units = sample_with_replacement(unique units at levels[0], same count, rng)
        FOR EACH sampled top unit occurrence:
            parent_copy = clone logical occurrence, preserving multiplicity
            current_parents = [original top unit]

            FOR level_index = 1 .. len(levels)-1:
                next_parents = []
                FOR EACH parent_occurrence IN current_parents:
                    children = observed children(parent_occurrence.original_id, levels[level_index])
                    REQUIRE children nonempty OR missingness policy declared
                    sampled_children = sample_with_replacement(children, len(children), rng)
                    attach sampled children to parent_copy occurrence
                    append next_parents, sampled_children
                current_parents = next_parents

            IF within_lowest_unit_policy == RESAMPLE_OBJECTS:
                resample cells/patches only for a within-specimen descriptive uncertainty target
            ELSE:
                include all lowest-level objects for each sampled region occurrence

            append sampled_records, materialize parent_copy subtree

        bootstrap_stats[b] = statistic_fn(sampled_records)

    interval = PercentileOrBCaInterval(bootstrap_stats, observed, design-aware jackknife)
    RETURN BootstrapResult(observed, bootstrap_stats artifact, interval, hierarchy, levels)
```

**Critical restriction:** resampling cells within one specimen cannot create patient-level uncertainty. The highest independent biological unit must be resampled first.

### 5.3 Paired patient permutation/sign-flip — **ESTABLISHED**

```text
FUNCTION PairedSpatialEndpointTest(pair_records, endpoint_fn, B, seed, alternative):
    REQUIRE every included pair has exactly the required conditions/timepoints
    differences = []
    FOR EACH pair:
        pre  = endpoint_fn(pair.pre)
        post = endpoint_fn(pair.post)
        REQUIRE axes and definitions identical
        append differences, post - pre

    observed = StudentizedMean(differences)
    null = []
    FOR b = 0 .. B-1:
        rng = DeterministicRng(DeriveSeed(seed, "paired_sign_flip", b))
        signs = independent Rademacher draws, one per patient pair
        null[b] = StudentizedMean(signs * differences)

    p = InclusivePlusOnePermutationP(observed, null, alternative)
    RETURN paired result with patient count, missing-pair exclusions, and curve-global inference if needed
```

### 5.4 Repeated-measures permutation using reduced-model residuals — **ADVANCED**

```text
FUNCTION RepeatedMeasuresFreedmanLane(
    records,
    full_model_design,
    reduced_model_design,
    subject_id,
    exchangeability_plan,
    B,
    seed
):
    REQUIRE hypothesis corresponds to columns present in full but absent in reduced model
    REQUIRE repeated observations for a subject remain in one exchangeability block

    fit_reduced = FitLinearOrGeneralizedModel(records.y, reduced_model_design)
    fitted_reduced = predict(fit_reduced)
    residuals = records.y - fitted_reduced
    observed = TestStatistic(FitModel(records.y, full_model_design), target_columns)

    FOR b = 0 .. B-1:
        rng = DeterministicRng(DeriveSeed(seed, "repeated_freedman_lane", b))
        residuals_b = PermuteOrSignFlipResidualBlocks(residuals, exchangeability_plan, rng)
        y_b = fitted_reduced + residuals_b
        null[b] = TestStatistic(FitModel(y_b, full_model_design), target_columns)

    RETURN permutation result with explicit residual-exchangeability assumptions
```

Use only when the residual permutation scheme is justified. For non-Gaussian or complex hierarchical outcomes, prefer an explicit mixed model or parametric bootstrap.

### 5.5 Multisite stratified inference and meta-analysis — **ESTABLISHED**

```text
FUNCTION MultisiteSpatialInference(site_patient_endpoints, effect_model, heterogeneity_model):
    FOR EACH site:
        fit site-specific patient-level effect using prespecified estimator
        obtain effect estimate beta_s and sampling covariance V_s

    IF effect_model == FIXED_EFFECT:
        beta = inverse_variance_weighted_mean(beta_s, V_s)
        variance = inverse(sum inverse(V_s))
    ELSE IF effect_model == RANDOM_EFFECT:
        estimate between-site heterogeneity tau using REML or Bayesian model
        beta = weighted mean with V_s + tau
        report prediction interval and heterogeneity diagnostics
    ELSE IF effect_model == ONE_STAGE_HIERARCHICAL:
        fit model with patient observations nested in site and site-level random effects

    run leave-one-site-out sensitivity
    run site-by-effect interaction assessment
    RETURN pooled effect, site effects, heterogeneity, and transfer limitations
```

## 6. Functional two-sample testing

### 6.1 Studentized functional permutation test — **ESTABLISHED**

```text
FUNCTION FunctionalTwoSamplePermutation(
    subject_curves,
    group_labels,
    common_axis,
    design,
    B,
    seed,
    test_statistic ∈ {L2, SUPREMUM, INTEGRATED_STUDENTIZED, ERL}
):
    REQUIRE one curve per biological unit or a documented within-unit aggregation
    REQUIRE identical axis and curve definition
    plan = BuildExchangeabilityPlan(design, subject_curves.hierarchy)

    observed_group_means = mean curves by group
    observed_difference = mean_A - mean_B
    variance_curve = pooled or Welch pointwise variance across biological units

    SWITCH test_statistic:
        L2: observed_stat = numerical_integral(observed_difference^2)
        SUPREMUM: observed_stat = max(abs(observed_difference / sqrt(variance_curve)))
        INTEGRATED_STUDENTIZED:
            observed_stat = integral((observed_difference^2) / variance_curve)
        ERL:
            defer to joint curve ranking

    FOR b = 0 .. B-1:
        labels_b = RestrictedShuffle(group_labels, plan, seeded_rng)
        compute difference curve and selected statistic

    IF ERL:
        RETURN ExtremeRankLengthGlobalEnvelope(observed_difference, null_difference_curves, eligible, alpha)
    ELSE:
        p = plus-one permutation p-value from scalar null statistic
        pointwise bands are descriptive unless generated by a valid simultaneous procedure
        RETURN FunctionalTestResult
```

### 6.2 Maximum-T family across multiple endpoints — **ESTABLISHED**

```text
FUNCTION MaxTMultipleEndpointPermutation(endpoint_matrix_by_patient, design, B, seed):
    observed_t = studentized group contrast for every endpoint
    FOR b:
        permute at allowed biological unit
        t_b = studentized contrast vector
        max_abs_b = max(abs(t_b))
    adjusted_p_j = (1 + count(max_abs_b >= abs(observed_t_j))) / (B + 1)
    RETURN observed_t, adjusted_p, max-T critical threshold
```

## 7. Kernel and distance-based two-sample tests

### 7.1 Maximum mean discrepancy — **ESTABLISHED**

```text
FUNCTION PatientLevelMMD(
    fingerprints,
    group_labels,
    kernel_spec,
    design,
    B,
    seed,
    unbiased = true
):
    REQUIRE one fingerprint per independent biological unit
    REQUIRE all preprocessing and kernel hyperparameters fixed before test cohort evaluation
    K = BuildKernelMatrix(fingerprints, kernel_spec)

    indices_A, indices_B = groups
    IF unbiased:
        mmd2 = mean(K_ij for i != j in A)
             + mean(K_ij for i != j in B)
             - 2 * mean(K_ij for i in A, j in B)
    ELSE:
        mmd2 = biased V-statistic

    plan = BuildExchangeabilityPlan(design, hierarchy)
    FOR b:
        labels_b = RestrictedShuffle(group_labels, plan, rng)
        null[b] = recompute MMD using same K and labels_b

    p = plus-one one-sided-high p-value
    RETURN {mmd2, p, kernel digest, null distribution artifact}
```

**Kernel policy:** bandwidth selection must be trained or prespecified. A median heuristic computed using the final combined test cohort changes the tested procedure and must be declared.

### 7.2 Energy distance — **ESTABLISHED**

```text
FUNCTION PatientLevelEnergyDistance(fingerprints, group_labels, metric, design, B, seed):
    REQUIRE metric is of negative type for the standard energy-test interpretation
    D = pairwise_distance_matrix(fingerprints, metric)
    A, Bidx = group indices

    energy = 2 * mean(D[a,b] for a in A, b in Bidx)
           - mean(D[a,a2] for a,a2 in A)
           - mean(D[b,b2] for b,b2 in Bidx)

    FOR b:
        labels_b = RestrictedShuffle(group_labels, design plan, rng)
        null[b] = energy statistic from same D

    p = plus-one one-sided-high p-value
    RETURN result with metric and preprocessing provenance
```

### 7.3 Spatially aware MMD using set/curve kernels — **ADVANCED**

```text
FUNCTION SpatialFingerprintKernel(fingerprint_i, fingerprint_j, component_specs):
    total = 0
    FOR EACH component spec:
        xi, xj = aligned component values or curves
        d = component-specific distance with physical axis and uncertainty weighting
        total += spec.weight * exp(-d^2 / (2 * spec.bandwidth^2))
    RETURN total

FUNCTION SpatialMMD(...):
    build kernel from versioned SpatialFingerprint components
    call PatientLevelMMD
    perform component ablations and leave-one-site-out sensitivity
```

## 8. Genuine equivalence and noninferiority

### 8.1 Two one-sided tests for equivalence — **ESTABLISHED**

```text
FUNCTION TOSTEquivalence(
    biological_unit_effects,
    lower_margin_delta_L,
    upper_margin_delta_U,
    alpha,
    design,
    variance_method
):
    REQUIRE margins were prespecified and scientifically justified
    REQUIRE lower_margin_delta_L < upper_margin_delta_U
    estimate = design-appropriate mean difference or model contrast
    se, df = design-appropriate uncertainty at biological-unit level

    t_lower = (estimate - lower_margin_delta_L) / se
    p_lower = P(T_df >= t_lower)            # reject effect <= lower margin

    t_upper = (estimate - upper_margin_delta_U) / se
    p_upper = P(T_df <= t_upper)            # reject effect >= upper margin

    equivalent = (p_lower < alpha) AND (p_upper < alpha)
    ci = two-sided confidence interval at level 1 - 2*alpha
    ASSERT_INTERNAL equivalent == (ci entirely inside [delta_L, delta_U]) under matching assumptions

    RETURN EquivalenceResult(estimate, se, ci, margins, p_lower, p_upper, equivalent)
```

### 8.2 Paired functional equivalence — **ADVANCED**

```text
FUNCTION FunctionalEquivalenceBand(patient_difference_curves, margin_curve, alpha, B, seed):
    REQUIRE margin_curve prespecified over physical axis
    mean_diff = pointwise patient mean
    bootstrap or permutation simultaneous band = max-deviation distribution at patient level
    equivalent_at_all_scales = lower_band > -margin_curve AND upper_band < margin_curve for all eligible scales
    RETURN simultaneous band, global decision, scales failing margin
```

### 8.3 Noninferiority — **ESTABLISHED**

```text
FUNCTION NoninferiorityTest(effect_estimate, se, direction, margin_delta, alpha, df):
    REQUIRE margin prespecified and sign convention explicit
    IF higher_is_better:
        H0 boundary = -margin_delta
        statistic = (effect_estimate + margin_delta) / se
        p = upper-tail probability
        noninferior = lower one-sided (1-alpha) confidence bound > -margin_delta
    ELSE:
        H0 boundary = +margin_delta
        use mirrored test
    RETURN result; never label as equivalence unless both one-sided equivalence nulls are rejected
```

### 8.4 Bootstrap equivalence for irregular estimands — **ADVANCED**

```text
FUNCTION BootstrapEquivalence(estimator, data, hierarchy, margins, B, seed):
    boot = HierarchicalBootstrap(hierarchy, levels, estimator, B, seed)
    ci = calibrated bootstrap interval appropriate to estimator smoothness
    equivalent = ci entirely within margins
    report sensitivity to interval method and finite-sample limitations
```

---

# Part III — Bayesian spatial modeling

## 9. Hierarchical mixed models and partial pooling

### 9.1 Generalized hierarchical spatial mixed model — **ESTABLISHED/ADVANCED**

```text
FUNCTION BuildHierarchicalMixedModel(data, formula, hierarchy, likelihood, spatial_component = NONE):
    ValidateCohortHierarchy(hierarchy)
    REQUIRE observation unit and biological unit explicitly declared

    # Example linear predictor
    # eta_i = X_i beta + Z_patient_i u_patient + Z_site_i u_site + spatial(s_i)

    model.parameters += beta ~ weakly informative prior
    FOR EACH random-effect level L in formula:
        z_L ~ Normal(0, I)
        sigma_L ~ prior constrained positive
        u_L = Cholesky(correlation_L) * z_L * sigma_L
        use non-centered parameterization when weakly identified

    IF spatial_component != NONE:
        attach GP, GMRF, CAR, or spatially varying coefficient field

    y_i ~ likelihood(link_inverse(eta_i), dispersion)
    generated quantities:
        patient/site effects
        marginal and conditional predictions
        variance partition
        posterior contrasts

    RETURN BayesianModelIR
```

### 9.2 Partial pooling summary

```text
FUNCTION SummarizePartialPooling(posterior, group_effect_parameter):
    FOR EACH group g:
        raw_estimate_g = unpooled estimate if computable
        posterior_mean_g = mean posterior group effect
        shrinkage_g = 1 - posterior_variance_g / approximate_unpooled_variance_g
        interval_g = posterior credible interval
    RETURN group summaries with warning that shrinkage is model-dependent, not a quality score
```

### 9.3 Hierarchical meta-analysis across cohorts/sites — **ESTABLISHED**

```text
FUNCTION BayesianRandomEffectsMetaAnalysis(site_effects, site_standard_errors, covariates):
    beta_global ~ prior
    tau ~ half-normal or penalized-complexity prior
    gamma ~ prior for site-level covariates
    FOR site s:
        theta_s ~ Normal(beta_global + covariates_s * gamma, tau)
        observed_effect_s ~ Normal(theta_s, site_standard_error_s)
    fit posterior
    generate posterior predictive effect for a new site
    RETURN global, site-specific, heterogeneity, and prediction distributions
```

## 10. Gaussian processes and Matérn fields

### 10.1 Exact Gaussian-process regression — **ESTABLISHED**

```text
FUNCTION ExactGaussianProcessRegression(X, coordinates, y, kernel_spec, noise_spec, priors):
    REQUIRE n is below exact-GP memory/time threshold
    REQUIRE coordinates have declared dimension and physical units

    beta ~ prior
    kernel_parameters theta ~ transformed priors
    sigma_noise ~ prior

    K = KernelMatrix(coordinates, coordinates, theta)
    Ky = K + NoiseCovariance(noise_spec, sigma_noise) + jitter * I
    L = Cholesky(Ky)

    log_likelihood = -0.5 * ||L^{-1}(y - X beta)||^2
                     - sum(log(diag(L)))
                     - n/2 * log(2*pi)

    infer beta, theta, noise parameters using HMC/NUTS or optimization + Laplace

    FUNCTION Predict(new_coordinates, X_new, posterior_draw):
        K_star = Kernel(new, training)
        K_ss = Kernel(new, new)
        alpha = solve_cholesky(Ky, y - X beta)
        mean = X_new beta + K_star alpha
        covariance = K_ss - K_star solve_cholesky(Ky, transpose(K_star))
        add observation noise only for predictive observations, not latent field
        RETURN multivariate normal prediction

    RETURN posterior + predictive operator
```

**Complexity:** time `O(n^3)`, memory `O(n^2)`.

### 10.2 Matérn covariance kernel — **ESTABLISHED**

```text
FUNCTION MaternKernel(distance_r, sigma, range_parameter, smoothness_nu, dimension):
    REQUIRE sigma > 0, range > 0, nu > 0
    IF distance_r == 0: return sigma^2
    scaled = sqrt(2*nu) * distance_r / range_parameter
    return sigma^2 * 2^(1-nu) / Gamma(nu) * scaled^nu * ModifiedBesselK(nu, scaled)
```

Record the exact range convention; different software packages use different parameterizations.

### 10.3 Multi-output linear model of coregionalization — **ADVANCED**

```text
FUNCTION MultiOutputGP(outputs, coordinates, latent_process_count, kernels, loadings_prior):
    FOR latent process q = 1..Q:
        f_q(s) ~ GP(0, K_q)
    loading matrix A ~ sparsity/identifiability prior
    FOR output m:
        latent_mean_m(s) = X_m(s) beta_m + sum_q A[m,q] * f_q(s)
        y_m(s) ~ modality-specific likelihood(latent_mean_m(s))
    impose sign/order constraints or post-process rotational ambiguity
    RETURN joint model and cross-covariance implied by A and K_q
```

## 11. Sparse and scalable Gaussian processes

### 11.1 Variational inducing-point GP — **ESTABLISHED/ADVANCED**

```text
FUNCTION VariationalInducingPointGP(data, kernel, m_inducing, inducing_init, likelihood):
    Z = InitializeInducingLocations(data.coordinates, m_inducing, inducing_init)
    Kuu = Kernel(Z, Z) + jitter I

    variational q(u) = Normal(m, S) in whitened coordinates
    prior p(u) = Normal(0, Kuu)

    FUNCTION ELBO(minibatch):
        FOR observations i in minibatch:
            compute q(f_i) from q(u), Kuf, and conditional GP equations
            expected_log_likelihood += E_q(f_i)[log p(y_i | f_i)]
        scale expected likelihood by n / minibatch_size
        kl = KL(q(u) || p(u))
        RETURN expected_log_likelihood - kl

    optimize kernel, likelihood, inducing locations, m, and S using stochastic gradients
    use natural gradients for Gaussian variational parameters when supported
    run multiple initializations
    compare against exact GP on small fixtures
    RETURN variational posterior with approximation status
```

**Complexity:** approximately `O(n m^2 + m^3)`, memory `O(nm + m^2)` or minibatch-bounded.

### 11.2 Deterministic training conditional / predictive process — **ADVANCED**

```text
FUNCTION LowRankPredictiveProcess(coordinates, knots, kernel):
    Kmm = K(knots, knots)
    Knm = K(coordinates, knots)
    low_rank_cov = Knm * inverse(Kmm) * transpose(Knm)
    residual_variance = diagonal(Knn - low_rank_cov)
    optionally add diagonal correction
    RETURN low-rank field representation
```

Do not call this exact GP inference. Validate oversmoothing and underrepresented local variation.

### 11.3 Nearest-neighbor Gaussian process — **ESTABLISHED/ADVANCED**

```text
FUNCTION BuildNNGP(coordinates, ordering, m_neighbors, kernel):
    order coordinates deterministically or by a declared space-filling strategy
    FOR i in ordered points:
        predecessors = points before i
        N_i = m nearest predecessors under physical metric
        C_NN = K(N_i, N_i)
        C_iN = K(i, N_i)
        B_i = C_iN * inverse(C_NN)
        F_i = K(i,i) - B_i * transpose(C_iN)
        REQUIRE F_i > numerical_tolerance
        store neighbor indices, B_i, F_i
    RETURN sparse directed conditional graph

FUNCTION NNGPLogDensity(field_w, plan):
    total = 0
    FOR i in order:
        residual = w_i - B_i w_{N_i}
        total += NormalLogDensity(residual; 0, F_i)
    RETURN total
```

**Complexity:** roughly `O(n m^3)` preprocessing and `O(n m^2)` repeated density work, with small fixed `m`.

### 11.4 SPDE Matérn approximation — **ESTABLISHED/ADVANCED**

```text
FUNCTION BuildSPDEMaternField(window, mesh_spec, alpha, kappa, tau):
    mesh = ConstrainedTriangulation(window, max_edge, cutoff, boundary_extension)
    REQUIRE mesh resolves holes, narrow interfaces, and anisotropic units

    basis = piecewise linear finite-element basis on mesh
    C = mass matrix
    G = stiffness matrix

    # For common alpha=2 case, schematic precision:
    Q = tau^2 * (kappa^4 * C + 2*kappa^2 * G + G * inverse_lumped(C) * G)
    apply boundary condition and identifiability constraints

    projection_A = evaluate basis at observation coordinates
    latent_weights w ~ GMRF(0, Q)
    field_at_observations = A * w

    RETURN mesh artifact, sparse precision, projection operator
```

Mesh sensitivity and boundary extension are part of the scientific result provenance.

## 12. CAR, SAR, GMRF, BYM, and BYM2

### 12.1 Spatial weights validation — **FOUNDATIONAL**

```text
FUNCTION ValidateSpatialWeights(W, region_ids, policy):
    REQUIRE square dimensions equal region count
    REQUIRE finite nonnegative off-diagonal weights unless signed model explicitly allows otherwise
    REQUIRE diagonal self-weights match policy
    IF symmetry required: REQUIRE W == transpose(W)
    identify disconnected components and islands
    apply declared row-standardization or preserve raw weights
    compute digest
    RETURN ValidatedSpatialWeights
```

### 12.2 Proper CAR field — **ESTABLISHED**

```text
FUNCTION ProperCAR(W, rho, tau):
    Wv = ValidateSpatialWeights(W, ...)
    D = diagonal(row_sums(Wv))
    REQUIRE rho lies inside interval ensuring Q = tau * (D - rho W) positive definite
    Q = tau * (D - rho W)
    u ~ NormalPrecision(0, Q)
    RETURN u
```

### 12.3 Intrinsic CAR field — **ESTABLISHED, improper prior**

```text
FUNCTION IntrinsicCAR(W, tau, constraints):
    D = diagonal(row_sums(W))
    Q = tau * (D - W)          # rank deficient by connected component
    impose sum-to-zero constraint per connected component
    handle islands with separate iid prior or explicit exclusion
    u ~ constrained improper GMRF(Q)
    RETURN u with rank deficiency and constraints recorded
```

### 12.4 SAR outcome/error model — **ESTABLISHED**

```text
FUNCTION SpatialAutoregressiveModel(y, X, W, model_type, priors):
    ValidateSpatialWeights(W)
    rho ~ prior constrained so I - rho W is nonsingular

    IF model_type == LAG:
        (I - rho W) y = X beta + epsilon
        include log_abs_determinant(I - rho W) in likelihood
    ELSE IF model_type == ERROR:
        y = X beta + u
        (I - rho W) u = epsilon
        implied covariance = sigma^2 * inverse(I-rho W) * inverse(transpose(I-rho W))

    fit with sparse linear algebra
    RETURN direct/indirect/total impacts only under a declared causal or descriptive interpretation
```

### 12.5 BYM disease-mapping model — **ESTABLISHED**

```text
FUNCTION BYMModel(counts_y, expected_E, X, W):
    eta_i = log(E_i) + X_i beta + u_i + v_i
    u = ICAR(W, tau_u)
    v_i ~ Normal(0, tau_v^-1)
    y_i ~ Poisson(exp(eta_i)) or appropriate overdispersed likelihood
    RETURN posterior risks and separate structured/unstructured components
```

### 12.6 BYM2 reparameterization — **ESTABLISHED**

```text
FUNCTION BYM2Model(counts_y, expected_E, X, scaled_ICAR_Q, priors):
    REQUIRE ICAR precision scaled so typical marginal variance is approximately 1
    sigma ~ interpretable prior on total latent standard deviation
    phi ∈ [0,1] ~ prior controlling structured variance fraction

    u_star ~ scaled ICAR with sum-to-zero constraints
    v ~ Normal(0, I)
    combined_i = sigma * (sqrt(phi) * u_star_i + sqrt(1-phi) * v_i)
    eta_i = log(E_i) + X_i beta + combined_i
    y_i ~ Poisson(exp(eta_i))
    RETURN posterior sigma, phi, risks, component fields
```

### 12.7 General sparse GMRF field — **ESTABLISHED/ADVANCED**

```text
FUNCTION GMRFLogDensity(x, Q, constraints):
    REQUIRE Q symmetric positive definite on constrained subspace
    return 0.5 * log_determinant_constrained(Q)
           - 0.5 * transpose(x) Q x
           + constant on constrained dimension
```

## 13. Spatially varying coefficients

### 13.1 GP or GMRF coefficient fields — **ADVANCED**

```text
FUNCTION SpatiallyVaryingCoefficientModel(y, X_global, Z_spatial, coordinates, likelihood):
    beta_global ~ prior
    FOR each spatially varying predictor k:
        b_k(s) = b0_k + delta_k(s)
        delta_k ~ GP, SPDE-GMRF, or CAR field with shrinkage prior
    eta_i = X_global_i beta_global + sum_k Z_spatial[i,k] * b_k(s_i)
    y_i ~ likelihood(eta_i)

    require centering constraints to separate global and spatial components
    perform posterior map uncertainty and multiplicity-aware summaries
    RETURN coefficient field posterior artifacts
```

## 14. Bayesian inference engines

### 14.1 Hamiltonian Monte Carlo — **ESTABLISHED**

```text
FUNCTION RunHMC(log_posterior, gradient, initial_state, mass_matrix, step_size, L, draws, seed):
    q = unconstrained initial_state
    FOR draw = 1 .. draws:
        p ~ Normal(0, mass_matrix)
        current_H = -log_posterior(q) + kinetic(p)
        q_proposed, p_proposed = Leapfrog(q, p, step_size, L, gradient)
        proposed_H = -log_posterior(q_proposed) + kinetic(p_proposed)
        accept with probability min(1, exp(current_H - proposed_H))
        IF accepted: q = q_proposed
        record energy error and constraint transform Jacobians
    RETURN chain artifact
```

### 14.2 No-U-Turn Sampler — **ESTABLISHED**

```text
FUNCTION RunNUTS(log_posterior, gradient, warmup, draws, chains, seed):
    FOR each chain with independent deterministic seed:
        initialize dispersed unconstrained state
        during warmup:
            adapt step size by dual averaging toward target acceptance
            adapt diagonal/dense/block mass matrix from warmup windows
        FOR each retained draw:
            sample momentum
            slice variable or multinomial trajectory selection
            recursively build balanced binary leapfrog tree
            stop when trajectory makes a U-turn or max depth reached
            select valid state from trajectory
            record divergence if Hamiltonian error exceeds threshold
        store chain without dropping divergent draws
    RETURN fit + adaptation and divergence diagnostics
```

### 14.3 Annealed sequential Monte Carlo — **ESTABLISHED/ADVANCED**

```text
FUNCTION RunAnnealedSMC(prior, likelihood, N_particles, ess_target, rejuvenation_kernel, seed):
    particles theta_i ~ prior
    log_weights_i = 0
    beta = 0

    WHILE beta < 1:
        choose next beta' > beta so conditional ESS reaches target, capped at 1
        log_weights_i += (beta' - beta) * log_likelihood(theta_i)
        normalize weights stably
        accumulate log normalizing constant estimate

        IF ESS(weights) below resample threshold:
            ancestors = systematic_resample(weights, deterministic seed)
            particles = particles[ancestors]
            reset weights

        FOR each particle:
            apply MCMC/HMC rejuvenation targeting prior * likelihood^beta'
        beta = beta'

    RETURN weighted posterior particles, evidence estimate, ESS path, ancestry artifact
```

### 14.4 Mean-field/full-rank variational inference — **ESTABLISHED approximation**

```text
FUNCTION RunVariationalInference(model, family, optimizer, max_steps, seed):
    q_phi(theta) = selected variational family in unconstrained space
    FOR step:
        epsilon ~ base distribution
        theta = reparameterize(phi, epsilon)
        elbo_sample = log_joint(theta) - log_q_phi(theta)
        gradient = automatic differentiation of Monte Carlo ELBO
        phi = optimizer.update(phi, gradient)
        monitor smoothed ELBO and gradient norm
    run multiple starts
    importance-resample or PSIS-correct when feasible
    compare moments/coverage with HMC on reduced fixtures
    RETURN approximation artifact, never labeled exact posterior
```

### 14.5 Laplace approximation — **ESTABLISHED approximation**

```text
FUNCTION RunLaplaceApproximation(log_joint, initial):
    theta_hat = robust optimizer argmax log_joint
    REQUIRE gradient norm small
    H = negative Hessian of log_joint at theta_hat
    REQUIRE H positive definite on unconstrained identifiable space
    covariance = inverse(H)
    q(theta) = Normal(theta_hat, covariance)
    optionally apply transformations/delta method or numerical marginal correction
    RETURN approximate posterior + Hessian conditioning diagnostics
```

### 14.6 INLA-style nested Laplace inference — **ESTABLISHED specialized approximation**

```text
FUNCTION RunInlaStyleApproximation(latent_gaussian_model):
    REQUIRE latent field is Gaussian/GMRF conditional on low-dimensional hyperparameters
    REQUIRE sparse precision available

    construct integration grid/design over hyperparameters theta
    FOR each theta support point:
        find mode x_hat(theta) of latent field conditional posterior
        compute sparse Hessian/precision at mode
        approximate p(theta | y) using Laplace ratio
        approximate marginals p(x_i | theta, y) by Gaussian or simplified/full Laplace
    numerically integrate conditional marginals over theta
    RETURN marginal posterior artifacts, integration diagnostics, and small-problem HMC comparison
```

## 15. Model diagnostics, comparison, and calibration

### 15.1 PSIS-LOO — **ESTABLISHED**

```text
FUNCTION PSISLOO(pointwise_log_likelihood_draws):
    REQUIRE likelihood contributions correspond to a scientifically valid held-out unit
    FOR observation/unit i:
        raw_importance_ratios = exp(-log_lik_draws[:, i])
        fit generalized Pareto to largest ratios
        smooth tail ratios and truncate if required
        k_hat_i = estimated Pareto shape diagnostic
        elpd_i = stable log weighted predictive density
    elpd_loo = sum(elpd_i)
    se = standard error using appropriate independent unit, not cells when patients are units
    flag high k_hat and trigger exact refits or K-fold CV
    RETURN LOOResult(elpd, se, k diagnostics, pointwise artifact)
```

### 15.2 Bayesian model comparison — **ESTABLISHED with restrictions**

```text
FUNCTION CompareBayesianModels(models, data, heldout_unit):
    FOR each model:
        compute PSIS-LOO or grouped K-fold predictive score at heldout_unit
        verify same observations, likelihood target, and data preprocessing
    compute pairwise elpd differences and uncertainty
    optionally compute stacking weights from pointwise predictive densities
    do not select solely by in-sample fit or posterior predictive p-values
    RETURN predictive comparison and diagnostic warnings
```

### 15.3 Simulation-based calibration — **ESTABLISHED validation procedure**

```text
FUNCTION SimulationBasedCalibration(model, inference_algorithm, R, L_posterior_draws, seed):
    ranks = map parameter_name -> []
    failures = []

    FOR r = 0 .. R-1:
        rng = DeterministicRng(DeriveSeed(seed, "sbc", r))
        theta_true ~ model.prior(rng)
        y_sim ~ model.simulator(theta_true, rng)
        fit = inference_algorithm(model, y_sim, seed_r)

        IF fit invalid/nonconverged:
            record failure; continue without pretending valid rank
        posterior_draws = obtain L exchangeable posterior draws
        FOR each scalar monitored quantity g(theta):
            rank = count(g(draw) < g(theta_true)) + randomized tie handling
            append ranks[g], rank

    assess rank uniformity with discrete-uniform diagnostics and autocorrelation correction
    assess coverage, z-scores, shrinkage, and failure rate
    RETURN SBCArtifact(ranks, ECDF/envelope summaries, failures)
```

### 15.4 Prior sensitivity — **ESTABLISHED workflow**

```text
FUNCTION RunPriorSensitivity(base_model, data, prior_grid):
    FOR each scientifically plausible prior alternative:
        refit or importance-reweight only when diagnostics support reweighting
        compare target posterior summaries, predictive performance, and decision quantities
    RETURN sensitivity table; flag conclusions that change materially
```

---

# Part IV — Bayesian point processes

## 16. Inhomogeneous Poisson processes

### 16.1 Log-linear inhomogeneous Poisson process — **ESTABLISHED**

```text
FUNCTION InhomogeneousPoissonLogLikelihood(points, window, covariate_fields, beta, quadrature):
    # log lambda(s) = X(s) beta + offset(s)
    REQUIRE all points lie in window
    REQUIRE quadrature weights cover exact window and sum to window measure

    event_term = 0
    FOR each point x_i:
        event_term += X(x_i) dot beta + offset(x_i)

    integral = 0
    FOR each quadrature node q_j with weight w_j:
        eta_j = X(q_j) dot beta + offset(q_j)
        integral += w_j * exp(eta_j)

    RETURN event_term - integral
```

```text
FUNCTION FitBayesianInhomogeneousPoisson(patterns, formula, prior, quadrature_spec):
    FOR each pattern p:
        quadrature_p = BuildWindowQuadrature(p.window, quadrature_spec)
        add log likelihood above
    beta ~ prior
    fit using HMC/NUTS, Laplace, or INLA-style method
    posterior predictive simulate Poisson process from lambda(s)
    RETURN posterior intensity surfaces and residual diagnostics
```

### 16.2 Berman–Turner quadrature approximation — **ESTABLISHED approximation**

```text
FUNCTION BuildBermanTurnerData(points, window, dummy_points, weights):
    nodes = observed points ∪ dummy points
    FOR each node j:
        y_j = 1 / weight_j if node is observed else 0
        response representation chosen so weighted Poisson GLM approximates point-process likelihood
    RETURN weighted quadrature table
```

The artifact must record quadrature resolution and convergence under refinement.

## 17. Log-Gaussian Cox processes

### 17.1 Gridded LGCP — **ESTABLISHED/ADVANCED**

```text
FUNCTION BuildGriddedLGCP(points, window, grid, covariates, field_prior):
    cells = IntersectRegularGridWithWindow(grid, window)
    FOR cell j:
        area_j = exact intersection measure
        count_j = number of points in cell
        covariates_j = area-weighted or center-evaluated covariates by declared rule

    beta ~ prior
    latent field z ~ Gaussian field prior (GP, FFT approximation, or GMRF)
    eta_j = covariates_j beta + z_j
    count_j ~ Poisson(area_j * exp(eta_j))

    RETURN BayesianModelIR plus grid/window intersection artifact
```

### 17.2 SPDE LGCP — **ESTABLISHED/ADVANCED**

```text
FUNCTION BuildSPDE_LGCP(points, window, mesh, covariates):
    field = BuildSPDEMaternField(window, mesh.spec, alpha, kappa, tau)
    integration_nodes = mesh vertices or higher-order quadrature nodes
    A_event = project field basis to event points
    A_quad  = project field basis to integration nodes

    log_likelihood = sum_i [X(x_i) beta + A_event_i w]
                   - sum_j weight_j * exp(X(q_j) beta + A_quad_j w)

    priors on beta and SPDE hyperparameters
    fit using HMC or INLA-style sparse latent Gaussian inference
    RETURN posterior intensity field and mesh sensitivity diagnostics
```

### 17.3 LGCP posterior prediction

```text
FUNCTION SimulateLGCPPosteriorPredictive(posterior_draw, window, discretization, seed):
    draw latent field z(s) from posterior predictive field
    lambda(s) = exp(X(s) beta + z(s))
    upper_bound = certified or adaptively refined bound on lambda within cells
    simulate Poisson counts per cell or use thinning
    sample locations conditionally inside each cell according to lambda
    RETURN replicated point pattern and approximation diagnostics
```

## 18. Neyman–Scott cluster processes

### 18.1 Thomas cluster process simulator — **ESTABLISHED**

```text
FUNCTION SimulateThomasProcess(window, kappa_parent, mu_offspring, sigma_um, seed):
    expanded_window = MinkowskiDilate(window, truncation_radius_for_sigma)
    parents ~ HomogeneousPoissonProcess(expanded_window, kappa_parent)
    offspring = []
    FOR parent c:
        N_c ~ Poisson(mu_offspring)
        FOR j = 1..N_c:
            displacement ~ Normal2D(0, sigma_um^2 I)
            x = c + displacement
            IF x in window: append offspring, x
    RETURN offspring, latent parents, truncation metadata
```

### 18.2 Matérn cluster process simulator — **ESTABLISHED**

```text
FUNCTION SimulateMaternClusterProcess(window, kappa_parent, mu_offspring, radius_um, seed):
    expanded_window = MinkowskiDilate(window, radius_um)
    parents ~ Poisson(expanded_window, kappa_parent)
    FOR parent:
        N ~ Poisson(mu_offspring)
        FOR child:
            r = radius_um * sqrt(Uniform(0,1))
            angle = Uniform(0, 2*pi)
            x = parent + [r cos(angle), r sin(angle)]
            retain if x in window
    RETURN observed children and latent parents
```

### 18.3 Bayesian latent-parent cluster inference — **ADVANCED**

```text
FUNCTION FitLatentParentClusterModel(observed_points, window, cluster_family, priors, inference):
    parameters kappa, mu, scale ~ priors
    latent parents C are unknown point pattern in expanded window
    latent allocation a_i maps each observed point to parent

    initialize C and allocations using clustering-informed but stochastic initializer

    MCMC iteration:
        1. update allocations a_i using conditional probabilities under displacement kernel
        2. birth/death/move parent proposals with reversible-jump acceptance
        3. update kappa, mu, scale conditional on C and allocations
        4. account for unobserved offspring and boundary truncation
        5. retain label-invariant summaries, not parent labels

    diagnose mixing in parent count, cluster scale, and intensity
    posterior predictive check K, g, nearest-neighbor, component sizes, and window-edge summaries
    RETURN posterior parameter and latent-cluster artifacts
```

### 18.4 Minimum-contrast or composite-likelihood alternative — **ESTABLISHED approximation**

```text
FUNCTION FitClusterProcessMinimumContrast(observed_K_or_g, theoretical_curve(theta), weights, range):
    objective(theta) = integral_range weights(r) * [transform(observed(r)) - transform(theoretical(theta,r))]^2 dr
    theta_hat = constrained optimizer objective
    uncertainty = patient-level bootstrap or parametric bootstrap, not naive cell bootstrap
    RETURN approximate estimator with method explicitly named minimum-contrast
```

## 19. Strauss and Gibbs interaction processes

### 19.1 Strauss model sufficient statistics — **ESTABLISHED**

```text
FUNCTION StraussStatistics(pattern, interaction_radius_R):
    n = number of points
    s_R = number of unordered point pairs with distance <= R
    RETURN {n, s_R}

# Density relative to unit-rate Poisson process:
# f(x | beta, gamma) ∝ beta^n(x) * gamma^s_R(x), usually 0 <= gamma <= 1
```

### 19.2 Papangelou conditional intensity

```text
FUNCTION StraussPapangelou(u, pattern, beta, gamma, R):
    neighbors = count points in pattern within distance R of u
    RETURN beta * gamma^neighbors
```

### 19.3 Birth–death Metropolis simulation — **ESTABLISHED**

```text
FUNCTION SimulateGibbsBirthDeath(window, papangelou, iterations, seed):
    pattern = initial valid pattern
    FOR t = 1..iterations:
        IF Bernoulli(0.5) == birth:
            u ~ Uniform(window)
            acceptance = min(1, window_measure * papangelou(u, pattern) / (n(pattern)+1))
            if accepted: insert u
        ELSE IF n(pattern) > 0:
            choose existing point x uniformly
            pattern_without = pattern \ {x}
            acceptance = min(1, n(pattern) / (window_measure * papangelou(x, pattern_without)))
            if accepted: delete x
    RETURN pattern after burn-in/thinning diagnostics
```

### 19.4 Berman–Turner pseudolikelihood fitting — **ESTABLISHED approximation**

```text
FUNCTION FitGibbsPseudolikelihood(pattern, window, conditional_intensity_features, quadrature):
    nodes = observed + dummy quadrature nodes
    FOR node u_j:
        compute local sufficient-statistic increment t(u_j, pattern)
        construct weighted Poisson regression row
    maximize approximate log pseudolikelihood
    estimate uncertainty with sandwich/parametric bootstrap where justified
    RETURN explicitly labeled pseudolikelihood fit
```

### 19.5 Exchange MCMC for doubly intractable posterior — **ADVANCED**

```text
FUNCTION ExchangeMCMC_Gibbs(observed_pattern, prior, unnormalized_density_h, exact_or_controlled_simulator, draws, seed):
    theta = initial parameter
    FOR iteration:
        theta_prime ~ proposal(theta)
        auxiliary_pattern y ~ model(theta_prime) using exact or sufficiently controlled internal sampler

        # Normalizing constants cancel in exchange ratio
        log_alpha = log prior(theta_prime) - log prior(theta)
                  + log proposal(theta | theta_prime) - log proposal(theta_prime | theta)
                  + log h(observed | theta_prime) - log h(observed | theta)
                  + log h(y | theta) - log h(y | theta_prime)

        accept theta_prime with probability min(1, exp(log_alpha))
        record inner-simulation diagnostics

    RETURN posterior with explicit statement whether auxiliary draws are exact or approximate
```

### 19.6 Saturated Geyer interaction for clustering — **ESTABLISHED**

```text
FUNCTION GeyerSaturationStatistic(pattern, R, saturation_s):
    total = 0
    FOR point i:
        neighbors_i = count other points within R
        total += min(s, neighbors_i)
    RETURN total with convention matching chosen density definition
```

Use for attractive interaction only with a valid stable/saturated model; do not set a Strauss `gamma > 1` and assume a normalizable process.

## 20. Multitype and joint location–mark models

### 20.1 Multitype Gibbs model — **ADVANCED**

```text
FUNCTION MultitypePapangelou(u, proposed_type_k, pattern, baseline_fields, interaction_matrix, radii):
    log_lambda = baseline_fields[k](u)
    FOR each existing point (x_j, type_l):
        r = distance(u, x_j)
        log_lambda += PairPotential(type_k, type_l, r, interaction_matrix, radii)
    RETURN exp(log_lambda)
```

```text
FUNCTION FitMultitypeGibbs(...):
    impose symmetry or directionality according to scientific model
    regularize interaction matrix; encode identifiability constraints
    choose pseudolikelihood, Monte Carlo likelihood, or exchange MCMC explicitly
    adjust multiplicity for interpreted type-pair effects
    posterior predictive check cross-K, cross-g, type frequencies, and nearest-neighbor types
```

### 20.2 Joint location–categorical-mark model — **ADVANCED**

```text
FUNCTION BuildJointLocationMarkModel(points, marks, covariates, model_spec):
    location process:
        lambda_total(s) = exp(X_loc(s) beta_loc + spatial_field_loc(s))

    conditional mark model:
        P(mark_i = k | location_i, neighborhood_i, latent_fields)
            = softmax_k(X_mark_i beta_k + field_k(s_i) + neighborhood_effect_k(i))

    joint likelihood = point-process location likelihood
                     + conditional mark likelihood
    distinguish from independent random labeling by testing neighborhood/field effects
    RETURN model IR
```

### 20.3 Joint location–continuous-mark model — **ADVANCED**

```text
FUNCTION BuildJointContinuousMarkModel(points, marks_y, covariates):
    location intensity depends on latent field f_loc(s)
    continuous mark depends on f_mark(s) and optionally shared field f_shared(s)

    log lambda(s) = X_loc beta + a_loc * f_shared(s) + f_loc(s)
    g(E[y(s)])    = X_mark gamma + a_mark * f_shared(s) + f_mark(s)

    assign multivariate GP/GMRF prior to fields
    fit joint posterior
    quantify posterior correlation induced by shared field
    compare to separate models to assess whether joint structure is supported
```

### 20.4 High-dimensional embedding marks in point-process model — **EXPERIMENTAL**

```text
FUNCTION JointLocationEmbeddingLatentFactorModel(points, embeddings, K_factors):
    embeddings_i ≈ W z_i + noise
    latent factor fields z_k(s) follow GP/GMRF priors
    location intensity may depend on selected/shared z fields
    impose shrinkage and rotational identifiability
    infer using variational or HMC method depending size
    validate against simpler vector variogram and mark-correlation models
```

## 21. Replicated hierarchical point patterns

### 21.1 Hierarchical replicated LGCP — **ADVANCED**

```text
FUNCTION ReplicatedHierarchicalLGCP(patterns, hierarchy, covariates, shared_field_policy):
    global beta ~ prior
    population covariance hyperparameters theta_pop ~ prior
    patient random effects b_patient ~ distribution

    FOR each pattern p nested in patient j:
        IF shared_field_policy == INDEPENDENT_REPLICATE_FIELDS:
            z_p(s) ~ GP/GMRF with shared theta_pop
        ELSE IF SHARED_PLUS_REPLICATE:
            z_shared_j(s) + z_p(s), each with separate covariance
        log lambda_p(s) = X_p(s) beta + b_j + z terms
        observed pattern p ~ Poisson/Cox process in its own window

    fit jointly; do not concatenate patterns into one window
    RETURN population and replicate-specific intensity/interaction summaries
```

### 21.2 Hierarchical cluster-process parameters — **ADVANCED**

```text
FUNCTION ReplicatedClusterModel(patterns):
    log kappa_p ~ Normal(mu_kappa + patient/site effects, sigma_kappa)
    log mu_p    ~ Normal(mu_mu + patient/site effects, sigma_mu)
    log scale_p ~ Normal(mu_scale + patient/site effects, sigma_scale)
    pattern_p ~ Thomas or Matérn cluster process(parameters_p, window_p)
    fit via latent-parent MCMC or SBI
    RETURN partially pooled cluster parameters and patient-level contrasts
```

## 22. Posterior-predictive K/g diagnostics

### 22.1 Point-process posterior predictive envelope — **ESTABLISHED workflow**

```text
FUNCTION PosteriorPredictivePointProcessDiagnostics(fit, observed_patterns, summaries, S, seed):
    FOR posterior predictive draw s:
        theta_s = posterior draw
        FOR each replicate pattern p:
            x_rep = simulate process in exact observed window W_p
            FOR summary T in summaries:
                compute T_rep using same estimator, edge correction, and eligible axis as observed
    FOR each observed pattern and population aggregate:
        compare observed K, L, g, F, G, J, mark functions, and count summaries to predictive distributions
        use simultaneous envelopes for curves
    RETURN diagnostics; posterior predictive consistency is not proof of model truth
```

---

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

# Part VI — Registration, atlas mapping, and transport

## 31. Nonrigid and diffeomorphic registration

### 31.1 Multi-resolution variational nonrigid registration — **ESTABLISHED**

```text
FUNCTION MultiResolutionNonrigidRegistration(fixed_image, moving_image, masks, config):
    validate physical resolution, coordinate frames, orientation, and masks
    build Gaussian/image pyramid from coarse to fine in physical units
    transform = initialize rigid/affine transform or supplied prior

    FOR level from coarse to fine:
        fixed_l, moving_l = pyramid level
        deformation_parameters = upsample transform parameters

        REPEAT until convergence or iteration limit:
            warped = Warp(moving_l, deformation_parameters)
            similarity = CrossModalSimilarity(fixed_l, warped, config.metric)
            regularization = SmoothnessOrElasticEnergy(deformation_parameters)
            mask_penalty = invalid/outside penalties
            objective = similarity + lambda * regularization + mask_penalty
            gradient = differentiate objective
            update parameters using line search or trust region
            reject step if fold/Jacobian constraints violated when diffeomorphic mode required

        record objective, Jacobian determinant, landmark error, inverse consistency

    RETURN TransformArtifact + quality and uncertainty placeholders
```

Similarity options must be explicit: sum of squared differences is not appropriate across arbitrary stains; mutual information, normalized gradient fields, feature metrics, or learned metrics require independent validation.

### 31.2 Stationary-velocity diffeomorphic registration — **ESTABLISHED/ADVANCED**

```text
FUNCTION SVFDiffeomorphicRegistration(fixed, moving, regularization_operator_L, config):
    velocity field v = zero or initialized field

    FUNCTION ExponentiateVelocity(v):
        # scaling and squaring approximation to phi = exp(v)
        n = choose steps so ||v / 2^n|| is small
        phi(x) = x + v(x) / 2^n
        FOR k = 1..n:
            phi = Compose(phi, phi)
        RETURN phi

    REPEAT optimization iterations:
        phi = ExponentiateVelocity(v)
        warped = Warp(moving, phi)
        data_loss = metric(fixed, warped)
        reg_loss = 0.5 * inner(v, L v)
        total = data_loss + reg_weight * reg_loss
        gradient_v = adjoint/automatic derivative through scaling-and-squaring
        v = optimizer_step(v, gradient_v)
        monitor min/max Jacobian determinant and inverse consistency

    REQUIRE Jacobian determinant positive above tolerance for diffeomorphic claim
    RETURN phi, inverse exp(-v), velocity field, Jacobian maps, diagnostics
```

### 31.3 LDDMM geodesic shooting — **ADVANCED**

```text
FUNCTION LDDMMRegistration(source, target, kernel_K, metric, config):
    optimize initial momentum m0

    FUNCTION Shoot(m0):
        initialize phi_0 = identity, momentum m_0 = m0
        integrate EPDiff/Hamiltonian equations over t ∈ [0,1]
        velocity v_t = K * m_t
        d phi_t / dt = v_t ∘ phi_t
        update m_t by coadjoint dynamics
        RETURN phi_1 and kinetic energy

    objective(m0) = kinetic_energy(m0) + data_weight * metric(Warp(source, Shoot(m0).phi), target)
    optimize with adjoint gradient
    RETURN diffeomorphism, momentum, geodesic energy, diagnostics
```

## 32. Probabilistic registration and uncertainty propagation

### 32.1 Variational probabilistic diffeomorphic registration — **ADVANCED**

```text
FUNCTION ProbabilisticDiffeomorphicRegistration(fixed, moving, model, variational_family):
    prior p(v) = Gaussian field/GMRF smoothness prior
    likelihood p(fixed | Warp(moving, exp(v)), noise_model, missing_data_model)
    q_phi(v) = Gaussian or structured variational posterior

    optimize ELBO:
        sample epsilon
        v = reparameterize(q_phi, epsilon)
        phi = ExponentiateVelocity(v)
        elbo = log likelihood + log prior(v) - log q_phi(v)

    validate positive Jacobians for posterior samples
    calibrate uncertainty on simulated known deformations and held-out landmarks
    RETURN posterior over transforms, not only mean deformation
```

### 32.2 Landmark-based Bayesian deformation posterior — **ADVANCED**

```text
FUNCTION BayesianLandmarkRegistration(source_landmarks, target_landmarks, deformation_prior, noise_prior):
    latent transform phi ~ deformation_prior
    target_k ~ Normal(phi(source_k), landmark_noise_covariance_k)
    sample or approximate posterior p(phi, noise | landmarks)
    RETURN posterior transform draws and residual uncertainty
```

### 32.3 Transform uncertainty propagated to cells — **ESTABLISHED Monte Carlo workflow**

```text
FUNCTION PropagateTransformUncertainty(point_table, transform_posterior, downstream_fn, S, seed):
    outputs = []
    FOR s = 1..S:
        phi_s = draw transform posterior
        transformed_points_s = ApplyTransform(phi_s, point_table.coordinates)
        IF cell localization uncertainty exists:
            add draw from localization error model
        output_s = downstream_fn(transformed_points_s)
        append outputs, output_s

    summarize posterior/Monte Carlo distribution of downstream quantities
    separate transform uncertainty from biological sampling uncertainty
    RETURN uncertainty-propagated result + transform draw artifact
```

### 32.4 Delta-method approximation for small deformation uncertainty

```text
FUNCTION DeltaPropagateTransformUncertainty(endpoint_fn, transform_mean, transform_covariance):
    J = numerical_or_autodiff Jacobian of endpoint_fn with respect to transform parameters
    endpoint_mean ≈ endpoint_fn(transform_mean)
    endpoint_cov ≈ J * transform_covariance * transpose(J)
    validate against Monte Carlo on representative cases
    RETURN approximate uncertainty with linearization diagnostic
```

## 33. Probabilistic correspondence

### 33.1 Soft correspondence with outlier/dustbin state — **ADVANCED**

```text
FUNCTION ProbabilisticCellCorrespondence(source_cells, target_cells, transform_posterior, feature_model, constraints):
    FOR each transform draw or integrated approximation:
        FOR source i and feasible target j within gated radius:
            spatial_cost = expected Mahalanobis distance under transform/localization uncertainty
            feature_cost = negative log compatibility of morphology/labels/features
            section_cost = prior penalty from serial-section biology
            log_score_ij = -(spatial_cost + feature_cost + section_cost)
        add unmatched/dustbin state for every source and target
        compute doubly-stochastic or many-to-one soft assignment under declared constraints
    average assignments across transform uncertainty
    RETURN correspondence probabilities and entropy
```

**Claim restriction:** this is a probabilistic compatibility map. Serial sections generally do not observe the same physical cell, so output must not be called cell identity without independent lineage/correspondence evidence.

### 33.2 Sinkhorn soft assignment with dustbin

```text
FUNCTION EntropicSoftAssignment(cost_C, source_mass_a, target_mass_b, epsilon, dustbin_cost):
    augment C with dustbin row/column
    K = exp(-C / epsilon)
    initialize u=1, v=1
    REPEAT until marginal residual tolerance:
        u = a / (K v)
        v = b / (transpose(K) u)
    P = diag(u) K diag(v)
    RETURN P, marginal residuals, entropy, epsilon sensitivity
```

## 34. Optimal transport

### 34.1 Balanced entropic OT — **ESTABLISHED**

```text
FUNCTION SinkhornOT(a, b, cost_C, epsilon, tolerance, max_iter):
    REQUIRE a,b nonnegative and sums equal
    log-domain initialize f=0, g=0
    FOR iteration:
        f = epsilon * [log(a) - LogSumExp_j((g_j - C_ij)/epsilon)]
        g = epsilon * [log(b) - LogSumExp_i((f_i - C_ij)/epsilon)]
        periodically center potentials for stability
        compute marginal residuals
        stop when tolerance met
    P_ij = exp((f_i + g_j - C_ij)/epsilon)
    RETURN transport plan artifact, dual potentials, residuals, regularized cost
```

### 34.2 Partial optimal transport — **ESTABLISHED/ADVANCED**

```text
FUNCTION PartialOT(a, b, C, transported_mass_m, epsilon):
    REQUIRE 0 < m <= min(sum(a), sum(b))
    solve min_P <C,P> + epsilon * entropy(P)
    subject to P 1 <= a, transpose(P) 1 <= b, sum(P) = m

    implementation options:
        augmented dummy mass formulation
        or specialized partial Sinkhorn/projection algorithm

    RETURN P, unmatched source/target mass, sensitivity over m and epsilon
```

### 34.3 Unbalanced optimal transport — **ESTABLISHED/ADVANCED**

```text
FUNCTION UnbalancedSinkhorn(a, b, C, epsilon, tau_a, tau_b, tolerance):
    K = exp(-C / epsilon)
    exponent_a = tau_a / (tau_a + epsilon)
    exponent_b = tau_b / (tau_b + epsilon)
    u = ones_like(a); v = ones_like(b)

    REPEAT:
        u = (a / max(K v, tiny)) ^ exponent_a
        v = (b / max(transpose(K) u, tiny)) ^ exponent_b
        check generalized marginal/KL residual
    P = diag(u) K diag(v)
    RETURN P, transported mass, source/target marginal deviations, objective terms
```

### 34.4 Fused Gromov–Wasserstein alignment — **ADVANCED**

```text
FUNCTION FusedGromovWasserstein(
    source_features_X,
    target_features_Y,
    source_structure_Dx,
    target_structure_Dy,
    masses_a,
    masses_b,
    alpha,
    epsilon,
    initialization
):
    feature_cost M_ij = FeatureDistance(X_i, Y_j)
    P = initialization satisfying marginals

    REPEAT outer iterations:
        structural_gradient_ij =
            sum_{k,l} Loss(Dx_i,k, Dy_j,l) * P_k,l
        linearized_cost = alpha * M + (1-alpha) * structural_gradient
        Q = SinkhornOT(a, b, linearized_cost, epsilon)
        line_search gamma to reduce full nonconvex FGW objective
        P = (1-gamma) P + gamma Q
        stop on objective/plan change

    run multiple initializations
    RETURN best plans, objective decomposition, initialization sensitivity
```

FGW is nonconvex; a single solution is not proof of unique correspondence.

### 34.5 Partial/unbalanced FGW — **EXPERIMENTAL**

```text
FUNCTION PartialUnbalancedFGW(...):
    combine feature and relational distortion objective
    replace exact marginal constraints with partial mass or KL marginal penalties
    solve by alternating linearization and partial/unbalanced Sinkhorn subproblems
    quantify sensitivity to alpha, mass penalty, entropy, and initialization
    RETURN ensemble of plausible plans rather than one asserted map
```

## 35. Reference/query atlas mapping

### 35.1 Atlas construction — **ADVANCED**

```text
FUNCTION BuildSpatialAtlas(reference_samples, representation_spec, alignment_spec):
    validate cohort and modality provenance
    compute reference cell/region representations with uncertainty
    choose atlas support objects: prototypes, regions, domains, or barycenter—not necessarily cells

    IF physical anatomy is homologous and registration available:
        build coordinate-aware reference frame
    ELSE:
        build biological similarity atlas with no physical registration claim

    estimate prototype distributions and variability across reference patients
    create retrieval/transport indices
    RETURN versioned atlas artifact with training population and support limitations
```

### 35.2 Query mapping

```text
FUNCTION MapQueryToAtlas(query, atlas, method, uncertainty_spec):
    verify query is in atlas-supported modality/model/stain/domain
    compute frozen query representation

    SWITCH method:
        NEAREST_PROTOTYPE: probability-calibrated nearest prototype
        SOFT_CLASSIFIER: posterior over atlas domains
        PARTIAL_OT: allow unmatched query/reference mass
        UNBALANCED_OT: allow composition differences
        FGW: align features and internal geometry

    propagate embedding, segmentation, and registration uncertainty
    compute unmatched/OOD mass and mapping entropy
    RETURN probabilistic mapping, not asserted cell identity
```

### 35.3 Atlas validation

```text
FUNCTION ValidateAtlasMapping(atlas, heldout_reference_patients, perturbations):
    leave-one-patient/site-out mapping
    measure domain/region recovery where labels exist
    measure stability under cell subsampling, missing types, geometry deformation, stain/scanner shift
    calibrate mapping probabilities
    evaluate unmatched mass for novel biology
    RETURN atlas model card
```

---
# Part VII — Graph and Higher-Order Tissue Mathematics

## 36. Canonical graph construction and operator contracts

Graph algorithms are meaningful only after fixing the graph. Every graph result therefore stores a complete `GraphOperatorSpec` and digest.

```text
TYPE GraphOperatorSpec:
    node_domain                  // cells, patches, regions, vessels, mixed entities
    coordinate_frame
    edge_rule                    // radius, kNN, mutual-kNN, Delaunay, learned, anatomical
    physical_radius_um OPTIONAL
    k OPTIONAL
    kernel                       // binary, Gaussian, adaptive, learned
    kernel_bandwidth_um OPTIONAL
    symmetrization               // union, intersection, average, max
    self_loops                   // included or excluded
    normalization                // unnormalized, random-walk, symmetric
    component_policy             // separate, block-diagonal, largest-only
    registration_resolution_policy
    uncertainty_weighting
    graph_digest

TYPE SparseGraphOperator:
    n_nodes
    edge_index[2, m]
    edge_weight[m]
    node_metadata
    degree[n]
    component_id[n]
    spec: GraphOperatorSpec

FUNCTION BuildCanonicalGraph(objects, spec, spatial_index, memory_budget):
    ValidateObjectIdentityAndCoordinates(objects)
    ValidateGraphSpec(spec)

    edges = EMPTY_CANONICAL_EDGE_SET

    SWITCH spec.edge_rule:
        RADIUS:
            FOR each node i:
                spatial_index.visit_within_radius(i, spec.physical_radius_um):
                    j = neighbor.index
                    IF j <= i: CONTINUE
                    weight = EvaluateGraphKernel(i, j, neighbor.distance_um, spec)
                    InsertCanonicalEdge(edges, i, j, weight, memory_budget)

        KNN:
            FOR each node i:
                neighbors = spatial_index.k_nearest(i, spec.k)
                FOR neighbor IN neighbors:
                    InsertDirectedCandidate(i, neighbor.index, weight)
            edges = SymmetrizeCandidates(edges, spec.symmetrization)

        DELAUNAY:
            triangulation = RobustDelaunay(objects.coordinates)
            edges = UniqueUndirectedTriangulationEdges(triangulation)

        ANATOMICAL:
            edges = BuildEdgesUnderCompartmentsAndBarrierRules(objects, spec)

        LEARNED:
            RequireFrozenExternalModelAndTrainingProvenance(spec)
            candidate_edges = GenerateSparseCandidateEdges(spatial_index)
            edges = ScoreAndThresholdCandidateEdges(candidate_edges, spec)

    IF spec.registration_resolution_policy == EXCLUDE_UNRESOLVED:
        edges = FilterEdgesAboveRegistrationResolution(edges, objects)
    ELSE IF spec.registration_resolution_policy == DOWNWEIGHT:
        edges = DownweightByRegistrationUncertainty(edges, objects)

    operator = BuildCompressedSparseOperator(edges, n_nodes)
    operator.component_id = ConnectedComponents(operator)
    operator.graph_digest = Hash(spec, object_ids, edges)
    RETURN operator
```

### 36.1 Laplacian construction

```text
FUNCTION BuildLaplacian(graph, normalization):
    W = SparseSymmetricAdjacency(graph)
    d_i = sum_j W_ij
    D = diag(d)

    SWITCH normalization:
        UNNORMALIZED:
            L = D - W
        RANDOM_WALK:
            REQUIRE d_i > 0 for active nodes
            L = I - D^{-1} W
        SYMMETRIC:
            REQUIRE d_i > 0 for active nodes
            L = I - D^{-1/2} W D^{-1/2}

    HandleIsolatedNodesByDeclaredPolicy(L, graph.spec)
    VerifyPSDWhenExpected(L)
    RETURN SparseLaplacian(L, graph_digest, normalization)
```

## 37. Sparse graph Fourier analysis — **ESTABLISHED / ADVANCED AT SCALE**

For an undirected graph with symmetric Laplacian `L = U Λ Uᵀ`, the graph Fourier transform of signal `x` is `x̂ = Uᵀx`. Exact full eigendecomposition is not acceptable for million-node graphs; use partial spectra or polynomial spectral filters.

### 37.1 Exact or partial graph Fourier transform

```text
FUNCTION GraphFourierTransform(signal x[n, p], laplacian L, spectrum_request):
    ValidateFiniteSignal(x)
    REQUIRE x.rows == L.n

    IF spectrum_request == FULL AND n <= configured_exact_limit:
        (lambda, U) = SymmetricEigendecomposition(L)
    ELSE:
        k = spectrum_request.lowest_k
        (lambda, U) = LanczosSmallestEigenpairs(L, k,
                                                tolerance,
                                                max_iterations,
                                                deterministic_start_seed)
        VerifyEigenResiduals(L, U, lambda)

    x_hat = U^T x
    energy_by_mode = RowSquaredNorm(x_hat)
    RETURN GraphSpectrum(lambda, x_hat, energy_by_mode,
                         exact = (k == n), residuals, graph_digest)
```

### 37.2 Graph-frequency band summaries

```text
FUNCTION SummarizeGraphFrequencyBands(spectrum, band_edges):
    ValidateMonotoneBandEdges(band_edges)
    FOR each band b:
        idx = {k : edge[b] <= lambda_k < edge[b+1]}
        IF idx empty:
            output[b] = InsufficientData("no eigenmodes in band")
        ELSE:
            output[b].energy = sum_k_in_idx energy_by_mode[k]
            output[b].energy_fraction = output[b].energy / total_energy
            output[b].mode_count = |idx|
    RETURN output
```

### 37.3 Graph Fourier null inference

```text
FUNCTION GraphSpectrumNullTest(signal, graph, design, permutations, alpha):
    plan = PrepareExchangeabilityPlan(design)
    observed = ComputeGraphFrequencySummary(signal, graph)

    null_curves = Matrix(permutations, observed.band_count)
    FOR b IN 0..permutations-1:
        permuted_signal = RestrictedPermuteRows(signal, plan, Seed(GRAPH_SPECTRUM, b))
        null_curves[b] = ComputeGraphFrequencySummary(permuted_signal, graph)

    envelope = ExtremeRankLengthEnvelope(observed.curve, null_curves, alpha)
    RETURN observed + envelope + scalar_low_frequency_p_value
```

## 38. Heat-kernel methods — **ESTABLISHED**

The heat operator is `H_t = exp(-tL)`. It summarizes diffusion over graph scale `t`, but `t` is graph-dependent and must be calibrated to physical distance using diffusion profiles or expected squared displacement.

### 38.1 Exact small-graph heat kernel

```text
FUNCTION ExactHeatKernel(L, times):
    (lambda, U) = SymmetricEigendecomposition(L)
    FOR t IN times:
        H_t = U diag(exp(-t * lambda)) U^T
    RETURN {H_t}
```

### 38.2 Matrix-free heat diffusion

```text
FUNCTION ApplyHeatKernel(signal x, L, t, approximation_spec):
    REQUIRE t >= 0
    IF n <= exact_limit:
        RETURN ExactSpectralApply(exp(-t * lambda), x)

    scaled_L, interval = ScaleSpectrumToMinusOneOne(L,
                                                    lambda_max_estimate = LanczosLargestEigenvalue(L))
    coefficients = ChebyshevCoefficients(f(lambda)=exp(-t*lambda),
                                         interval,
                                         approximation_spec.order)
    y = ChebyshevApply(scaled_L, x, coefficients)
    error_bound = BoundChebyshevApproximationError(coefficients, interval)
    RETURN ApproximateSignal(y, error_bound, algorithm="chebyshev_heat")
```

### 38.3 Heat-kernel signatures and diffusion distances

```text
FUNCTION HeatKernelSignature(L, times, node_subset=ALL):
    FOR each t:
        diag_H = EstimateOrComputeHeatKernelDiagonal(L, t)
        signature[:, t] = diag_H[node_subset]
    RETURN signature

FUNCTION DiffusionDistance(i, j, L, t):
    h_i = ApplyHeatKernel(delta_i, L, t)
    h_j = ApplyHeatKernel(delta_j, L, t)
    RETURN WeightedL2(h_i - h_j, stationary_measure)
```

## 39. Spectral graph wavelets — **ESTABLISHED MATHEMATICS; EXPERIMENTAL PATHOLOGY ENDPOINT**

Given a band-pass generating kernel `g` and low-pass `h`, spectral graph wavelets are `ψ_s = g(sL)` and scaling functions are `φ = h(L)`.

```text
TYPE SpectralWaveletSpec:
    scales[]
    bandpass_kernel g
    lowpass_kernel h
    chebyshev_order
    spectral_bound_policy
    normalization

FUNCTION SpectralGraphWaveletTransform(signal x, L, spec):
    lambda_max = EstimateLargestEigenvalue(L)
    ValidateWaveletKernelCoverage(spec, lambda_max)

    scaling = PolynomialSpectralFilter(L, x, h(lambda), spec.chebyshev_order)
    coefficients = []
    errors = []

    FOR s IN spec.scales:
        filter_function(lambda) = g(s * lambda)
        coeff_s, err_s = PolynomialSpectralFilter(L, x, filter_function,
                                                   spec.chebyshev_order)
        coefficients.append(coeff_s)
        errors.append(err_s)

    VerifyFrameBoundsOrRecordUnavailable(spec, lambda_max)
    RETURN GraphWaveletResult(scaling, coefficients, errors,
                              graph_digest, spec)
```

### 39.1 Wavelet energy and localization

```text
FUNCTION GraphWaveletEnergy(coefficients, node_groups OPTIONAL):
    FOR scale s:
        global_energy[s] = sum_i ||coefficients[s][i]||^2
        IF node_groups present:
            FOR group g:
                group_energy[g,s] = sum_i_in_g ||coefficients[s][i]||^2
    RETURN normalized energies and raw energies
```

## 40. Diffusion wavelets — **ADVANCED / EXPERIMENTAL**

Diffusion wavelets construct multiresolution bases by repeatedly compressing powers of a diffusion operator.

```text
FUNCTION BuildDiffusionWaveletTree(graph, tolerance, max_levels):
    T = BuildLazyDiffusionOperator(graph)       // e.g. I - L / lambda_max
    basis_0 = IdentityBasis(n)
    levels = []

    FOR j IN 0..max_levels-1:
        T_power = T^(2^j) represented in current basis
        (Q_j, R_j, retained_rank) = RankRevealingQR(T_power, tolerance)
        scaling_basis = Q_j[:, 0:retained_rank]
        detail_basis = OrthogonalComplementWithinPreviousBasis(scaling_basis)
        compressed_T = scaling_basis^T T_power scaling_basis

        levels.append({scaling_basis, detail_basis, compressed_T,
                       approximation_error})

        IF retained_rank stabilizes OR retained_rank == 1:
            BREAK
        T = compressed_T

    RETURN DiffusionWaveletTree(levels, graph_digest, tolerance)
```

```text
FUNCTION DiffusionWaveletTransform(signal, tree):
    current = signal
    FOR level IN tree.levels:
        detail[level] = level.detail_basis^T current
        current = level.scaling_basis^T current
    RETURN {coarse=current, detail}
```

## 41. Generic Chebyshev polynomial approximation — **ESTABLISHED**

```text
FUNCTION ChebyshevApply(A_scaled, X, coefficients c[0..K]):
    // Spectrum of A_scaled must lie in [-1, 1].
    T0 = X
    Y = 0.5 * c[0] * T0

    IF K == 0:
        RETURN Y

    T1 = A_scaled * X
    Y += c[1] * T1

    FOR k IN 2..K:
        Tk = 2 * A_scaled * T1 - T0
        Y += c[k] * Tk
        T0 = T1
        T1 = Tk

    RETURN Y
```

```text
FUNCTION AdaptiveChebyshevOrder(filter, spectral_interval, tolerance, max_order):
    FOR K IN increasing_orders:
        coefficients = ComputeChebyshevCoefficients(filter, interval, K)
        tail_bound = EstimateCoefficientTail(coefficients)
        IF tail_bound <= tolerance:
            RETURN K, coefficients, tail_bound
    RETURN InsufficientData("requested approximation tolerance not achieved")
```

## 42. Graph scattering — **ADVANCED / EXPERIMENTAL**

Graph scattering cascades wavelet modulus nonlinearities and global or group pooling. It is useful only when graph construction, scale definitions, and invariance are scientifically justified.

```text
FUNCTION GraphScattering(signal X, wavelet_bank, max_order, pooling):
    paths = [{path=[], signal=X}]
    outputs = []

    FOR order IN 0..max_order:
        next_paths = []
        FOR state IN paths:
            pooled = PoolGraphSignal(state.signal, pooling)
            outputs.append({path=state.path, value=pooled})

            IF order == max_order:
                CONTINUE

            FOR scale s IN wavelet_bank.scales:
                IF ViolatesFrequencyOrdering(state.path, s):
                    CONTINUE
                filtered = ApplyWavelet(wavelet_bank[s], state.signal)
                propagated = ElementwiseNormOrModulus(filtered)
                next_paths.append({path=state.path + [s], signal=propagated})
        paths = next_paths

    RETURN ScatteringFeatureVector(outputs, graph_digest, wavelet_spec)
```

```text
FUNCTION ValidateGraphScatteringStability(scattering_model, perturbations):
    FOR perturbation IN perturbations:
        perturbed_graph_or_signal = ApplyPerturbation(perturbation)
        delta = Norm(Scattering(perturbed) - Scattering(original))
        compare delta to perturbation magnitude and predefined tolerance
    RETURN stability_curve
```

## 43. Heterogeneous tissue graphs — **ADVANCED**

```text
TYPE HeterogeneousGraph:
    node_types                // cell, gland, vessel, nerve, compartment, patch, clone region
    node_tables_by_type
    edge_types                // cell-near-cell, cell-in-region, vessel-near-cell, etc.
    sparse_edges_by_relation
    coordinate_frames
    provenance

FUNCTION BuildHeterogeneousTissueGraph(modalities, relation_specs):
    ValidateStableCrossModalityIdentity(modalities)
    graph = EMPTY_HETEROGENEOUS_GRAPH

    FOR modality IN modalities:
        AddTypedNodes(graph, modality)

    FOR relation IN relation_specs:
        SWITCH relation.kind:
            SPATIAL_NEAR:
                edges = RadiusOrKnnRelation(relation.source_type,
                                            relation.target_type,
                                            relation.radius_um)
            CONTAINMENT:
                edges = PointInPolygonOrPatchContainment(relation)
            INTERFACE_CONTACT:
                edges = BoundaryContactRelation(relation)
            SHARED_CLONE:
                edges = ConnectByCloneAssignmentWithUncertainty(relation)
            LEARNED:
                edges = ExternalFrozenRelationModel(relation)
        AddTypedEdges(graph, relation.name, edges)

    ValidateNoIdentityLeakageOrImpossibleRelations(graph)
    RETURN graph
```

### 43.1 Relational message passing contract

```text
FUNCTION HeterogeneousMessagePassing(graph, node_features, relation_parameters):
    FOR layer l:
        messages_by_target = zeros
        FOR relation r = source_type -> target_type:
            FOR edge (i, j) IN graph.edges[r]:
                message = phi_r(node_features[source_type][i],
                                edge_features[r][i,j],
                                relation_parameters[r])
                Aggregate(messages_by_target[target_type][j], message,
                          relation_parameters[r].aggregation)
        FOR node_type t:
            node_features[t] = Update_t(node_features[t], messages_by_target[t])
    RETURN node_features
```

A learned heterogeneous graph model must be patient-held-out, provenance-complete, and separated from descriptive graph statistics.

## 44. Hypergraph methods — **ADVANCED / EXPERIMENTAL**

A hyperedge may encode a gland, niche, clone, patch, compartment, or higher-order cellular interaction.

```text
TYPE Hypergraph:
    incidence H[n_nodes, n_hyperedges] sparse
    hyperedge_weight W_e
    node_degree D_v
    hyperedge_degree D_e
    hyperedge_type
    provenance

FUNCTION BuildHypergraph(nodes, hyperedge_definitions):
    H = SparseIncidenceMatrix()
    FOR each definition e:
        members = ResolveMembersByGeometryLabelsOrImportedAssignment(e)
        REQUIRE |members| >= minimum_members
        FOR node i IN members:
            H[i,e] = MembershipWeight(i,e)
    ComputeDegrees(H)
    RETURN Hypergraph(H, weights, types, provenance)
```

### 44.1 Normalized hypergraph Laplacian

```text
FUNCTION HypergraphLaplacian(hypergraph):
    H = hypergraph.incidence
    W = diag(hypergraph.hyperedge_weight)
    Dv = diag(sum_e W_e * H_ie)
    De = diag(sum_i H_ie)
    Theta = Dv^{-1/2} H W De^{-1} H^T Dv^{-1/2}
    L_h = I - Theta
    RETURN L_h
```

```text
FUNCTION HypergraphSignalSmoothness(signal, L_h):
    RETURN trace(signal^T L_h signal) / max(trace(signal^T signal), epsilon)
```

## 45. Motif-based graph analysis — **ESTABLISHED GRAPH METHOD; ADVANCED PATHOLOGY USE**

```text
FUNCTION CountTypedMotifs(graph, motif_catalog, node_labels):
    counts = zeros(motif_catalog.size)
    FOR motif IN motif_catalog:
        plan = CompileMotifEnumerationPlan(motif, graph.sparsity_structure)
        FOR embedding IN EnumerateSubgraphEmbeddings(graph, plan):
            IF NodeAndEdgeTypesMatch(embedding, motif, node_labels):
                counts[motif] += MotifWeight(embedding)
    RETURN counts
```

```text
FUNCTION BuildMotifAdjacency(graph, motif):
    W_motif = sparse zeros(n,n)
    FOR each motif instance S:
        FOR unordered pair (i,j) IN S.anchor_nodes:
            W_motif[i,j] += 1
    RETURN W_motif
```

```text
FUNCTION MotifNullTest(graph, labels, motif_statistic, null_model, B):
    observed = motif_statistic(graph, labels)
    FOR b IN 0..B-1:
        null_graph_or_labels = SimulateDeclaredGraphNull(graph, labels, null_model, Seed(MOTIF,b))
        null[b] = motif_statistic(null_graph_or_labels)
    RETURN PermutationInference(observed, null)
```

## 46. Simplicial complexes and Hodge operators — **ADVANCED / EXPERIMENTAL**

```text
TYPE SimplicialComplex:
    simplices_by_dimension
    orientation
    boundary_matrix B[k]        // maps k-simplices to (k-1)-simplices
    filtration_value OPTIONAL
    provenance

FUNCTION BuildCliqueComplex(graph, max_dimension):
    simplices[0] = graph.nodes
    simplices[1] = graph.edges
    FOR k IN 2..max_dimension:
        simplices[k] = EnumerateCliquesOfSize(graph, k+1)
        AssignCanonicalOrientation(simplices[k])
    B = BuildBoundaryMatrices(simplices)
    VerifyBoundaryOfBoundaryZero(B)
    RETURN SimplicialComplex(simplices, B)
```

### 46.1 Hodge Laplacians

```text
FUNCTION HodgeLaplacian(complex, k):
    Bk = complex.boundary_matrix[k]
    Bkp1 = complex.boundary_matrix[k+1] IF exists
    L_lower = Bk^T Bk
    L_upper = Bkp1 Bkp1^T IF exists ELSE 0
    L_k = L_lower + L_upper
    RETURN {L_k, L_lower, L_upper}
```

### 46.2 Edge-flow decomposition

```text
FUNCTION HodgeDecomposeEdgeFlow(flow_on_edges, complex):
    L0 = B1 B1^T
    gradient_potential = SolveLeastSquares(L0, B1 * flow)
    gradient_component = B1^T * gradient_potential

    IF triangles exist:
        curl_potential = SolveLeastSquares(B2^T B2, B2^T * flow)
        curl_component = B2 * curl_potential
    ELSE:
        curl_component = 0

    harmonic_component = flow - gradient_component - curl_component
    VerifyOrthogonalityWithinTolerance()
    RETURN {gradient_component, curl_component, harmonic_component}
```

### 46.3 Simplicial signal filtering

```text
FUNCTION FilterKSimplicialSignal(signal, L_k, spectral_filter, approximation):
    RETURN PolynomialSpectralFilter(L_k, signal, spectral_filter, approximation.order)
```

## 47. Cellular complexes — **RESEARCH-ONLY UNTIL PATHOLOGY USE IS FIXED**

```text
FUNCTION BuildCellularComplex(segmented_tissue):
    cells_0 = junctions or selected landmarks
    cells_1 = interfaces or vessel/gland boundary arcs
    cells_2 = compartments, glands, clones, or tissue domains
    boundary_1 = OrientedIncidence(cells_1 -> cells_0)
    boundary_2 = OrientedIncidence(cells_2 -> cells_1)
    VerifyBoundaryOfBoundaryZero(boundary_1, boundary_2)
    RETURN CellularComplex(cells_0, cells_1, cells_2, boundary_matrices)
```

Do not promote a cellular-complex endpoint without a pathology interpretation and robustness under segmentation perturbation.

## 48. Graph-method validation suite

```text
FUNCTION ValidateGraphMathematicsSuite():
    // Independent exact fixtures
    compare Laplacians/eigenpairs against dense reference on small graphs
    compare Chebyshev filters against exact spectral filtering
    verify spectral graph wavelet frame behavior
    verify diffusion-wavelet compression error
    verify scattering stability under controlled graph perturbations
    verify hypergraph Laplacian against hand incidence matrices
    verify motif counts against exhaustive enumeration
    verify B_k * B_{k+1} == 0 for simplicial/cellular complexes
    verify Hodge decomposition reconstruction and orthogonality

    // Scientific stress tests
    vary radius, k, kernel bandwidth, normalization, barriers, and components
    quantify graph-choice sensitivity
    test registration and segmentation perturbations
    test fixed-density scaling and sparse-memory behavior
    test deterministic CPU/GPU parity where GPU is supported

    RETURN validation ledger entries per algorithm and operator spec
```

---

# Part VIII — Topology and Mathematical Morphology

## 49. Filtrations and persistent homology — **ESTABLISHED MATHEMATICS; ADVANCED PATHOLOGY USE**

### 49.1 Generic filtration contract

```text
TYPE Filtration:
    simplices ordered by nondecreasing filtration_value
    tie_break_order
    boundary_columns
    coefficient_field
    dimension_limit
    provenance

FUNCTION ValidateFiltration(filtration):
    REQUIRE every face appears no later than its coface
    REQUIRE finite filtration values
    REQUIRE deterministic tie order
    REQUIRE boundary-of-boundary zero
```

### 49.2 Standard matrix-reduction persistence algorithm

```text
FUNCTION PersistentHomology(filtration):
    ValidateFiltration(filtration)
    R = CopyBoundaryMatrixColumns(filtration)
    V = IdentityChangeOfBasis OPTIONAL
    low_to_column = EMPTY_MAP
    pairs = []

    FOR column j IN filtration order:
        WHILE R[j] is nonempty:
            i = LowestNonzeroRow(R[j])
            IF i NOT IN low_to_column:
                BREAK
            k = low_to_column[i]
            factor = Coefficient(R[j], i) / Coefficient(R[k], i)
            R[j] = R[j] - factor * R[k]        // over selected field
            IF representative_cycles_requested:
                V[j] = V[j] - factor * V[k]

        IF R[j] is nonempty:
            i = LowestNonzeroRow(R[j])
            low_to_column[i] = j
            birth = filtration.value[i]
            death = filtration.value[j]
            dimension = filtration.simplex_dimension[i]
            pairs.append((dimension, birth, death, i, j))
        ELSE:
            MarkAsPotentialEssentialBirth(j)

    FOR unpaired zero columns j:
        pairs.append((dimension(j), filtration.value[j], +infinity, j, NONE))

    RETURN PersistenceDiagramByDimension(pairs, optional_representatives)
```

Production code should use a proven persistence library or rigorously validated reduction backend; the pseudocode defines the contract and validation oracle.

## 50. Alpha complexes — **ESTABLISHED**

```text
FUNCTION BuildAlphaFiltration(points, max_alpha, dimension):
    REQUIRE Euclidean coordinates and valid dimensionality
    delaunay = DelaunayTriangulation(points)
    simplices = []

    FOR simplex sigma IN delaunay.simplices up to dimension:
        alpha_value = SquaredRadiusOfSmallestEmptyCircumsphere(sigma, delaunay)
        IF alpha_value <= max_alpha^2:
            AddSimplexAndFacesWithFiltrationValue(simplices, sigma, alpha_value)

    ResolveDegeneraciesByExactOrRobustPredicates()
    SortByFiltrationThenFaceOrder(simplices)
    RETURN Filtration(simplices, physical_scale = sqrt(alpha_value))
```

Weighted alpha complexes may incorporate cell radii or uncertainty only under a separate validated contract.

## 51. Witness complexes — **ADVANCED / EXPERIMENTAL**

Useful for subsampling very large point clouds, but approximation error and landmark selection must be recorded.

```text
FUNCTION BuildWitnessFiltration(points, landmark_spec, max_dimension, nu, max_scale):
    landmarks = SelectLandmarks(points, landmark_spec)   // farthest-point or stratified
    witnesses = points
    distance_table = NearestLandmarkDistances(witnesses, landmarks, max_needed)

    simplices = all vertices at filtration 0
    FOR candidate landmark simplex sigma up to max_dimension:
        value = MinimumWitnessRadiusSatisfyingNuWitnessCondition(sigma,
                                                                  witnesses,
                                                                  distance_table,
                                                                  nu)
        IF value <= max_scale:
            AddSimplexAndFaces(simplices, sigma, value)

    RETURN Filtration(simplices,
                      approximation={landmark_count, coverage_radius, nu})
```

## 52. Persistence landscapes — **ESTABLISHED**

For each finite persistence pair `(b,d)`, define a tent function `f(t)=max(0,min(t-b,d-t))`; the `k`th landscape is the `k`th largest tent value.

```text
FUNCTION PersistenceLandscape(diagram, grid, max_k):
    finite_pairs = RemoveEssentialAndZeroPersistencePairs(diagram)
    landscapes[max_k, |grid|] = 0

    FOR grid index q, t IN grid:
        values = []
        FOR (birth, death) IN finite_pairs:
            values.append(max(0, min(t - birth, death - t)))
        sort values descending
        FOR k IN 0..min(max_k, values.length)-1:
            landscapes[k,q] = values[k]

    RETURN landscapes with grid and norm convention
```

## 53. Persistence images — **ESTABLISHED**

```text
FUNCTION PersistenceImage(diagram, birth_grid, persistence_grid, kernel_bandwidth, weight_fn):
    image = zeros(len(birth_grid), len(persistence_grid))

    FOR finite pair (b,d) IN diagram:
        p = d - b
        IF p <= 0: CONTINUE
        weight = weight_fn(b,p)
        FOR pixels within kernel truncation radius:
            image[pixel] += weight * IntegratedGaussianOverPixel(center=(b,p),
                                                                 bandwidth=kernel_bandwidth)

    NormalizeByDeclaredPolicy(image)
    RETURN image with transform and pixel-area metadata
```

The weight function and bandwidth must be fitted only within training folds for predictive use.

## 54. Euler characteristic curves — **ESTABLISHED**

For a finite cell/simplicial complex, `χ = Σ_k (-1)^k N_k`.

```text
FUNCTION EulerCharacteristicCurve(filtration, thresholds):
    counts_by_dimension = zeros(max_dim+1)
    events = filtration.simplices sorted by value
    cursor = 0

    FOR threshold t IN thresholds ascending:
        WHILE cursor < events.length AND events[cursor].value <= t:
            k = events[cursor].dimension
            counts_by_dimension[k] += 1
            cursor += 1
        chi[t] = sum_k (-1)^k * counts_by_dimension[k]

    RETURN chi curve and simplex counts
```

## 55. Minkowski functionals and mathematical morphology — **ESTABLISHED FOR BINARY SETS**

In 2-D, the primary additive functionals are area, perimeter, and Euler characteristic; report conventions explicitly.

```text
FUNCTION MinkowskiFunctionals2D(binary_set, representation, scale):
    IF representation == POLYGON:
        area = ExactPolygonArea(binary_set)
        perimeter = ExactBoundaryLength(binary_set)
        euler = Components(binary_set) - Holes(binary_set)
    ELSE IF representation == RASTER:
        area = PixelAreaEstimator(binary_set, scale)
        perimeter = CalibratedCroftonPerimeter(binary_set, scale)
        euler = DigitalEulerCharacteristic(binary_set, connectivity_convention)

    RETURN {area, perimeter, euler,
            normalized_perimeter=perimeter/sqrt(area),
            conventions, uncertainty}
```

### 55.1 Dilation/erosion curves

```text
FUNCTION MorphologicalFunctionalCurve(set, radii_um):
    FOR r IN radii_um:
        dilated = MinkowskiDilation(set, disk_radius=r)
        eroded  = MinkowskiErosion(set, disk_radius=r)
        output.dilation[r] = MinkowskiFunctionals2D(dilated)
        output.erosion[r]  = MinkowskiFunctionals2D(eroded)
    RETURN output
```

## 56. Percolation and connectivity transitions — **ADVANCED / EXPERIMENTAL PATHOLOGY USE**

```text
FUNCTION ConnectivityTransition(points, radii_um, window):
    spatial_index = BuildSpatialIndex(points)
    union_find = InitializeDisjointSet(n)
    edge_events = GeneratePairDistanceEventsUpToMaxRadius(spatial_index, max(radii_um))
    sort edge_events by distance
    cursor = 0

    FOR r IN radii_um ascending:
        WHILE cursor < edge_events.length AND edge_events[cursor].distance <= r:
            union_find.union(edge.source, edge.target)
            cursor += 1

        component_sizes = union_find.component_sizes()
        largest_fraction[r] = max(component_sizes) / n
        n_components[r] = len(component_sizes)
        susceptibility[r] = sum_{components excluding largest} size^2 / n
        spans_window[r] = DetectBoundarySpanningComponent(union_find, points, window)

    critical_radius = EstimateTransitionRadius(largest_fraction,
                                               susceptibility,
                                               spans_window)
    RETURN curves + critical_radius + finite-size caveats
```

For cohort inference, compare patient-level transition summaries; do not treat each edge event as a replicate.

## 57. Topological two-sample comparison

```text
FUNCTION ComparePersistenceDistributions(group_A_diagrams, group_B_diagrams, design, metric):
    REQUIRE independent biological-unit diagrams or fingerprints

    distance_matrix = PairwiseDiagramDistance(all_diagrams,
                                              metric = bottleneck | Wasserstein | landscape_L2)
    statistic = EnergyStatisticFromDistanceMatrix(distance_matrix, group_labels)
    p_value = RestrictedPatientPermutation(statistic, design)

    RETURN statistic, p_value, distance_matrix_artifact, metric_spec
```

## 58. Stability under segmentation and scale perturbation

```text
FUNCTION TopologyStabilityLaboratory(base_segmentation,
                                     perturbation_generator,
                                     scales,
                                     repetitions):
    baseline_outputs = ComputeTopologyAndMorphology(base_segmentation, scales)
    results = []

    FOR r IN 0..repetitions-1:
        perturbed = perturbation_generator(base_segmentation, Seed(TOPOLOGY_PERTURB,r))
        outputs = ComputeTopologyAndMorphology(perturbed, scales)

        results.append({
            diagram_bottleneck = BottleneckDistance(outputs.diagram,
                                                     baseline_outputs.diagram),
            landscape_L2 = L2(outputs.landscape - baseline_outputs.landscape),
            euler_curve_Linf = MaxAbs(outputs.euler - baseline_outputs.euler),
            minkowski_relative_error = RelativeError(outputs.functionals,
                                                       baseline_outputs.functionals),
            critical_radius_shift = outputs.percolation_radius
                                    - baseline_outputs.percolation_radius
        })

    RETURN distributions, quantiles, failure rates, sensitivity flags
```

## 59. Topology/morphology validation suite

```text
FUNCTION ValidateTopologySuite():
    use hand complexes with known Betti numbers
    compare alpha-complex persistence to a trusted reference implementation
    verify landscape and persistence-image transforms against independent code
    verify Euler characteristic by both Betti numbers and alternating simplex counts
    compare polygon and high-resolution raster Minkowski functionals
    verify percolation curves on lattice and random geometric graph controls
    test stability under bounded coordinate and segmentation perturbations
    test memory scaling and sparse reduction behavior
    record essential-class and infinite-death policy explicitly
```

---
# Part IX — Multimodal Bayesian Models

## 60. Unified multimodal observation contract

```text
TYPE ModalityObservation:
    modality_id
    entity_level             // cell, patch, region, slide, specimen, patient
    entity_ids
    matrix_or_sparse_table
    feature_metadata
    likelihood_family        // Gaussian, Bernoulli, binomial, Poisson, NB, ordinal, categorical
    offset OPTIONAL
    censoring OPTIONAL
    measurement_status       // measured, imported_prediction, morphology_prediction
    batch_covariates
    spatial_coordinates OPTIONAL
    missingness_mask
    provenance

TYPE MultimodalModelDesign:
    modalities[]
    identity_hierarchy
    shared_latent_levels
    modality_specific_latent_levels
    spatial_prior_spec
    covariates
    random_effects
    missingness_model
    inference_backend
    maturity_tier
```

```text
FUNCTION ValidateMultimodalDesign(design):
    verify stable identity joins and entity levels
    prohibit joining cell-level and patient-level matrices without an explicit aggregation/link model
    distinguish measured from predicted modalities
    validate likelihood support and offsets
    validate missingness assumptions
    validate training/test boundary and leakage restrictions
    validate spatial coordinate frames
    RETURN compiled multimodal design
```

## 61. Probabilistic canonical correlation analysis — **ESTABLISHED**

For paired Gaussian views:

```text
z_i ~ Normal(0, I_k)
x_i ~ Normal(mu_x + W_x z_i, Psi_x)
y_i ~ Normal(mu_y + W_y z_i, Psi_y)
```

### 61.1 EM algorithm for pCCA

```text
FUNCTION FitProbabilisticCCA(X[n,dx], Y[n,dy], k, regularization):
    RequirePairedRowsAndFiniteValues(X,Y)
    standardization = FitStandardizationOnTrainingData(X,Y)
    Xc, Yc = ApplyStandardization(X,Y)
    O = ConcatenateColumns(Xc, Yc)

    Initialize W_x, W_y using classical CCA or small random values
    Initialize Psi_x, Psi_y as positive diagonal/block covariance

    REPEAT until convergence:
        W = StackRows(W_x, W_y)
        Psi = BlockDiagonal(Psi_x, Psi_y)
        M = I_k + W^T Psi^{-1} W
        M_inv = StableCholeskyInverse(M)

        // E-step
        FOR i IN 1..n:
            Ez[i] = M_inv W^T Psi^{-1} O[i]
            Ezz[i] = M_inv + Ez[i] Ez[i]^T

        // M-step
        S_oz = sum_i O[i] Ez[i]^T
        S_zz = sum_i Ezz[i]
        W = S_oz * inverse(S_zz + regularization * I)

        residual_cov = (1/n) * sum_i [
            O[i]O[i]^T - W Ez[i]O[i]^T - O[i]Ez[i]^T W^T + W Ezz[i] W^T
        ]
        Psi_x = ProjectToDeclaredNoiseStructure(residual_cov[x,x])
        Psi_y = ProjectToDeclaredNoiseStructure(residual_cov[y,y])
        EnforcePositiveDefinite(Psi_x, Psi_y)

        log_likelihood = GaussianMarginalLogLikelihood(O, W, Psi)
        CheckMonotoneOrNumericallyStableConvergence(log_likelihood)

    RETURN pCCAModel(parameters, posterior_scores=Ez,
                     standardization, diagnostics)
```

### 61.2 Bayesian pCCA

```text
FUNCTION FitBayesianPCCA(X,Y,k,priors,inference):
    model:
        W_x[:,j] ~ Normal(0, alpha_x[j]^{-1} I)
        W_y[:,j] ~ Normal(0, alpha_y[j]^{-1} I)
        alpha_*[j] ~ Gamma(a,b)                  // ARD
        noise precisions ~ Gamma or structured priors
        z_i ~ Normal(0,I)
        X,Y likelihoods as above

    posterior = RunSelectedBayesianInference(model, inference)
    diagnose factor sign/rotation non-identifiability
    align posterior factor draws before summaries
    RETURN posterior factors, loadings, cross-view predictions, diagnostics
```

## 62. Bayesian multiview factor model — **ESTABLISHED / ADVANCED**

General model for modalities `m=1..M`:

```text
Y_m[i,f] ~ Likelihood_m( inverse_link_m(mu_m[f] + Z[i,:] W_m[f,:]^T + covariates) )
```

with global/shared factors and optional group- or modality-specific factors.

### 62.1 Variational multiview factor fitting

```text
FUNCTION FitMultiviewFactorModel(observations, K_max, priors, design, VI_spec):
    compiled = ValidateMultimodalDesign(design)
    Initialize variational factors:
        q(Z), q(W_m), q(ARD_m), q(noise_m), q(random_effects)

    FOR iteration IN 1..VI_spec.max_iterations:
        minibatch = SelectEntityMinibatchPreservingHierarchy(compiled)

        FOR modality m:
            observed_entries = NonMissingEntries(minibatch, m)
            expected_natural_parameters = ComputeExpectedLinearPredictor(qZ, qW_m, ...)

            IF likelihood conjugate Gaussian:
                UpdateGaussianVariationalFactorsClosedForm()
            ELSE:
                local_bound = ConstructLikelihoodBoundOrReparameterizedEstimator(m)
                OptimizeModalityELBO(local_bound)

        UpdateSharedFactors(qZ)
        UpdateLoadings(qW_m)
        UpdateARDAndSpikeSlabIndicators()
        UpdateRandomEffectsAndCovariateCoefficients()
        ApplyIdentifiabilityConstraintOrPostHocFactorAlignment()

        ELBO = EstimateFullOrStochasticELBO()
        IF ConvergedWithStableHeldoutPredictiveScore(ELBO): BREAK

    PruneInactiveFactorsByPosteriorARDWithSensitivityCheck()
    RETURN model with posterior means/variances,
           factor activity per modality,
           variance explained,
           missing-value predictions,
           ELBO trace and calibration diagnostics
```

### 62.2 Grouped hierarchical factors

```text
FUNCTION AddHierarchicalFactorStructure(base_model, hierarchy):
    z_patient[p] ~ Normal(0, I)
    z_specimen[s] ~ Normal(A_patient z_patient[parent(s)], Sigma_specimen)
    z_region[r] ~ Normal(A_specimen z_specimen[parent(r)], Sigma_region)
    z_cell[i] ~ Normal(A_region z_region[parent(i)], Sigma_cell)

    modality observations attach to the appropriate entity level
    prohibit cell observations from directly acting as patient replicates
    RETURN hierarchical factor graph
```

## 63. Bayesian matrix factorization — **ESTABLISHED**

```text
FUNCTION BayesianMatrixFactorization(Y, mask, K, likelihood, priors, inference):
    model:
        U[i,k] ~ prior_U
        V[j,k] ~ prior_V
        eta[i,j] = bias_i + bias_j + dot(U[i,:], V[j,:])
        Y[i,j] ~ likelihood(link^{-1}(eta[i,j])) for mask[i,j]=observed

    posterior = RunSelectedBayesianInference(model, inference)
    align latent draws for sign/permutation symmetry
    imputed = posterior_predictive for missing entries
    RETURN U, V posterior, imputed uncertainty, heldout predictive diagnostics
```

### 63.1 Spatially regularized matrix factorization

```text
FUNCTION SpatialBayesianMatrixFactorization(Y, graph_or_coordinates, K, priors):
    FOR factor k:
        U[:,k] ~ GMRF(precision = tau_k * (L + epsilon*I))
        OR U[:,k] ~ GP(Matern parameters)
    V[:,k] ~ ARD prior
    observation likelihood as declared
    fit with HMC, VI, or Laplace depending dimensions
    RETURN spatial factor posteriors and uncertainty
```

## 64. Bayesian tensor factorization — **ADVANCED**

### 64.1 CP decomposition

For tensor `Y[i,j,t,...]`:

```text
Y_index ~ likelihood(sum_{k=1}^K product_mode A_mode[index_mode,k] + covariates)
```

```text
FUNCTION BayesianCPFactorization(tensor, mask, K_max, priors, inference):
    Initialize factor matrices A_m for each tensor mode m
    FOR component k:
        global_shrinkage[k] ~ multiplicative_gamma_process OR ARD
        A_m[:,k] ~ Normal(0, (global_shrinkage[k]*local_scale_m)^{-1})

    likelihood over observed tensor entries only
    posterior = RunSelectedBayesianInference()
    prune components with posterior shrinkage and validate rank sensitivity
    align component permutation/sign across draws
    RETURN factor posteriors, reconstructed tensor, missing entries, uncertainty
```

### 64.2 Tucker factorization

```text
FUNCTION BayesianTuckerFactorization(tensor, ranks, priors):
    core G[r1,...,rM] ~ shrinkage prior
    factors A_m[dim_m, r_m] ~ matrix-normal or structured prior
    eta = G ×1 A_1 ×2 A_2 ... ×M A_M
    fit using VI or HMC for tractable sizes
    RETURN posterior core, factors, reconstruction, diagnostics
```

## 65. Spatial latent-factor models — **ESTABLISHED / ADVANCED**

### 65.1 GP spatial factors

```text
FUNCTION FitSpatialLatentFactorModel(Y[n,p], coordinates, K, likelihoods, priors):
    FOR k IN 1..K:
        z_k(coordinates) ~ GP(0, Matern(theta_k))
    loadings W[p,K] ~ shrinkage/identifiability priors
    eta[i,j] = mu_j + sum_k W[j,k] z_k(s_i) + X_i beta_j
    Y[i,j] ~ likelihood_j(link_j^{-1}(eta[i,j]))

    choose exact GP, inducing GP, NNGP, or SPDE backend by n and geometry
    fit posterior
    align factors across draws
    RETURN spatial factor maps, loadings, covariance ranges, uncertainty
```

### 65.2 GMRF/SPDE spatial factors

```text
FUNCTION FitSPDESpatialFactorModel(Y, mesh, K, priors):
    FOR factor k:
        w_k ~ GMRF(Q_spde(kappa_k, tau_k, mesh))
        z_k(s_i) = A_projection[i,:] w_k
    observation model as spatial factor model
    use Laplace/INLA-style or variational backend
    RETURN mesh field posterior and projected cell/region factors
```

### 65.3 Multiresolution spatial factors

```text
FUNCTION FitMultiresolutionSpatialFactors(Y, spatial_bases_by_scale, K_by_scale):
    eta = fixed effects
    FOR scale q:
        coefficients_q ~ shrinkage prior with scale-specific precision
        eta += basis_q * coefficients_q * loadings_q^T
    fit posterior
    report variance and predictive contribution by physical scale
    RETURN scale-resolved factor model
```

## 66. Missing-modality inference — **ADVANCED**

Missing-modality prediction must distinguish missing at random, structurally absent, and missing not at random.

```text
FUNCTION InferMissingModalities(model, observations, missingness_spec):
    ValidateMissingnessMechanism(missingness_spec)

    IF missingness_spec == MAR_OR_STRUCTURAL:
        condition posterior on all observed modalities
        FOR missing modality m:
            draws[m] = PosteriorPredictive(model, modality=m,
                                           condition_on=observed)

    ELSE IF missingness_spec == MNAR:
        define missingness indicator R_m
        model P(R_m | latent factors, observed covariates, possibly Y_m)
        perform sensitivity analysis over nonidentified parameters
        draws[m] = joint posterior predictive under each sensitivity setting

    RETURN distributions, intervals, calibration, and missingness assumptions
```

### 66.1 Modality dropout training

```text
FUNCTION TrainModalityRobustInference(model, training_data, dropout_distribution):
    FOR optimization step:
        modality_mask = SampleModalityDropoutPattern(dropout_distribution,
                                                     preserve_required_anchor=true)
        optimize ELBO/predictive objective using only retained modalities
        include consistency loss between full- and partial-modality posteriors
    validate every clinically plausible missingness pattern on heldout patients
    RETURN robust model with per-pattern calibration
```

## 67. Joint morphology–IHC–omics–clone–clinical model — **FRONTIER / EXPERIMENTAL**

A single monolithic likelihood is usually brittle. Compile a modular probabilistic graph with shared and modality-specific latent variables.

```text
FUNCTION CompileJointPathologyModel(project, model_spec):
    ValidateMultimodalDesign(model_spec)

    // Hierarchical latent state
    z_patient[p] ~ Normal(0,I)
    z_specimen[s] ~ Normal(Ap*z_patient[parent(s)], Sigma_s)
    z_region[r] ~ SpatialPriorConditionedOnSpecimen(z_specimen[parent(r)])
    z_cell[i] ~ Normal(Ar*z_region[parent(i)], Sigma_cell)

    // Morphology embeddings
    cell_embedding[i] ~ GaussianOrHeavyTailedDecoder(z_cell[i], batch_covariates)
    patch_embedding[q] ~ Decoder_patch(z_region[parent(q)], patch_scale[q], technical_covariates)

    // Measured IHC
    continuous_ihc[i,m] ~ RobustGaussianOrOrdinalLikelihood(
                              f_ihc(z_cell[i], stain_batch, registration_uncertainty))

    // Spatial omics
    counts[spot,g] ~ NegativeBinomial(
                         library_size[spot] * exp(f_gene(z_region[spot], gene_factors[g])))

    // Clone/CNA labels or uncertain probabilities
    clone[i] ~ Categorical(softmax(f_clone(z_cell[i], z_region[parent(i)])))
    IF imported clone uncertainty exists:
        connect latent clone to observed noisy clone assignment through confusion matrix

    // Clinical outcome at patient level
    outcome[p] ~ DeclaredPatientLevelLikelihood(
                     baseline_covariates[p],
                     z_patient[p],
                     prespecified aggregated spatial summaries[p])

    // Optional interfaces/graphs
    add spatial interactions or GMRF priors only through explicit operator specs

    RETURN ModelIR with measurement-status annotations and claim limits
```

### 67.1 Inference strategy

```text
FUNCTION FitJointPathologyModel(model_ir, data, inference_plan):
    IF dimensionality tractable:
        use blocked HMC/NUTS for global parameters + elliptical/specialized updates for latent fields
    ELSE:
        initialize with structured VI
        optionally refine key global/posterior blocks with HMC or SMC
        validate approximation against smaller exact subproblems

    monitor modality-specific posterior predictive checks
    monitor cross-modality prediction calibration
    monitor latent-factor identifiability and prior dominance
    run patient-held-out predictive evaluation for clinical claims
    RETURN posterior artifact, diagnostics, and result maturity tier
```

## 68. Multimodal model comparison and ablation

```text
FUNCTION CompareMultimodalModels(models M0..M5, cohort_splits, metric_spec):
    FOR outer patient-held-out fold:
        FOR model M in M0..M5:
            fit all preprocessing, latent dimensions, priors, and tuning inside training fold
            evaluate heldout predictive density, calibration, and task metrics
            record failures/nonconvergence

    compute paired fold- and patient-level incremental comparisons:
        M1-M0, M2-M0, M3-M1, M3-M2, M4-M3, M5-M4
    use patient-level bootstrap/permutation for uncertainty
    report whether gain persists across site/scanner/stain/tumor subgroups
    RETURN ablation report without causal interpretation
```

## 69. Multimodal Bayesian validation suite

```text
FUNCTION ValidateMultimodalBayesianSuite():
    simulate from pCCA and recover canonical subspace
    simulate shared and modality-specific factors with missing values
    verify factor recovery up to sign/rotation/permutation
    calibrate posterior intervals and posterior predictive distributions
    verify spatial range/loadings under GP and GMRF factor models
    test MNAR sensitivity rather than pretending identification
    compare exact small-model HMC with VI/Laplace approximations
    test modality-dropout robustness
    perform patient-held-out real-data validation
    test site/stain/scanner confounding and negative-control modalities
    record all failed fits and prior-sensitivity results
```

---
# Part X — Generative Tissue Modeling and Simulation-Based Inference

## 70. Simulator contract

```text
TYPE TissueSimulatorSpec:
    simulator_id
    version
    dimensionality
    observation_window
    entity_types
    state_variables
    parameter_space with units and bounds
    initial_condition_distribution
    stochastic_sources
    numerical_solver
    discretization
    stopping_rule
    observation_model
    measurement_noise
    random_seed_namespace
    differentiability_status
    provenance

TYPE SimulatedTissue:
    latent_trajectory OPTIONAL
    final_latent_state
    observed_cells/fields/images
    parameter_draw
    solver_diagnostics
    conservation_residuals
    event_log
    provenance

FUNCTION ValidateSimulator(spec):
    validate dimensionality, units, bounds, initial and boundary conditions
    validate solver stability/CFL constraints where applicable
    validate positivity/conservation invariants
    validate observation model and coordinate frame
    validate deterministic replay from seed
    RETURN compiled simulator
```

## 71. Reaction–diffusion models — **ESTABLISHED MATHEMATICS; EXPERIMENTAL BIOLOGICAL MODEL**

General form for species/state vector `u(x,t)`:

```text
∂u/∂t = D ∇²u + R(u, x, t; θ) + stochastic forcing
```

### 71.1 Finite-volume/finite-element reaction–diffusion solver

```text
FUNCTION SimulateReactionDiffusion(mesh, u0, diffusion D, reaction R, theta, T, dt_spec):
    ValidateMeshAndBoundaryConditions(mesh)
    u = u0
    t = 0
    trajectory = []

    WHILE t < T:
        dt = ChooseStableTimeStep(mesh, D, R, u, dt_spec)

        // Strang operator splitting
        u = SolveReactionODE(u, R, theta, dt/2,
                             method = adaptive_stiff_solver)
        u = SolveDiffusionStep(mesh, u, D, dt,
                               boundary_conditions,
                               method = implicit_CrankNicolson_or_FEM)
        u = SolveReactionODE(u, R, theta, dt/2)

        IF stochastic_forcing enabled:
            u += SampleSpatialNoiseConsistentWithDiscretization(dt)

        EnforceOrCheckPhysicalConstraints(u)
        RecordMassBalanceAndResiduals()
        t += dt
        trajectory.append_if_requested(t,u)

    observations = ApplyObservationModel(u, theta)
    RETURN SimulatedTissue(trajectory, u, observations, diagnostics)
```

### 71.2 Pattern-formation diagnostics

```text
FUNCTION AnalyzeReactionDiffusionPattern(simulation, expected_instability_spec):
    compute linear stability of homogeneous equilibrium
    calculate predicted unstable wavelength band
    compare simulated spectrum/domain scale with prediction
    quantify boundary and discretization sensitivity
    RETURN mechanistic consistency report
```

## 72. Ecological competition and evolutionary games — **ADVANCED / EXPERIMENTAL**

### 72.1 Spatial Lotka–Volterra field model

```text
FOR species a:
    ∂n_a/∂t = D_a ∇²n_a
               + r_a n_a (1 - sum_b alpha_ab n_b / K_a)
               - treatment_effect_a(x,t) n_a
               + stochastic_process_noise
```

```text
FUNCTION SimulateSpatialCompetition(mesh, densities0, parameters, treatment, T):
    compile coupled reaction–diffusion system
    integrate with positivity-preserving splitting
    record coexistence, exclusion, front speed, and extinction events
    sample cells/marks through observation model
    RETURN trajectory and observations
```

### 72.2 Agent-based competition

```text
FUNCTION SimulateAgentCompetition(window, initial_agents, parameters, T):
    spatial_index = DynamicSpatialIndex(initial_agents)
    event_queue = EMPTY
    t = 0

    WHILE t < T:
        FOR agent i:
            local_context = QueryNeighborsAndResources(i, spatial_index)
            rates[i] = {
                birth = BirthRate(agent_i, local_context, parameters),
                death = DeathRate(...),
                move  = MovementRate(...),
                switch_state = PhenotypeSwitchRate(...)
            }

        total_rate = SumAllRates(rates)
        IF total_rate == 0: BREAK
        dt ~ Exponential(total_rate)
        event = SampleEventProportionalToRate(rates)
        ApplyEvent(event, agents, spatial_index)
        t += dt
        RecordEventIfRequested()

    RETURN observed agent pattern and latent event history
```

## 73. Vascular transport and distance-to-resource models — **ADVANCED**

### 73.1 Advection–diffusion–reaction around vessels

```text
∂c/∂t = ∇·(D(x)∇c) - v(x)·∇c - uptake(c, cells; θ) + vessel_source(x,t)
```

```text
FUNCTION SimulateVascularTransport(mesh, vessel_graph, cells, parameters, T):
    map vessel sources and boundary exchange to mesh
    compute/admit flow field v(x) from external hemodynamic model or declared approximation
    FOR time steps:
        solve advection with conservative stabilized scheme
        solve diffusion implicitly
        apply nonlinear uptake/reaction
        enforce nonnegative concentration
        update cell states if coupled
    RETURN concentration field, gradients, hypoxic regions, solver diagnostics
```

### 73.2 Statistical resource-distance model

```text
FUNCTION FitDistanceToResourceModel(cell_outcome, resource_geometry, covariates, hierarchy):
    distances = SignedOrUnsignedDistance(cell_coordinates, resource_geometry)
    model outcome with spline/GP of distance plus compartment and patient effects
    include resource-density and accessibility covariates
    fit hierarchical posterior
    posterior predictive check across patients and vessels/resources
    RETURN distance-response posterior, not a transport-causal claim
```

## 74. Growth-front models — **ESTABLISHED MATHEMATICS; EXPERIMENTAL TUMOUR MODEL**

### 74.1 Fisher–KPP-like density front

```text
∂n/∂t = D∇²n + r n(1 - n/K)
```

```text
FUNCTION SimulateGrowthFront(mesh, n0, D, r, K, T):
    integrate reaction–diffusion equation
    extract level-set fronts at predefined density thresholds
    estimate front speed, curvature, branching, and anisotropy
    compare numerical speed with theoretical planar-wave control where applicable
    RETURN density trajectory and front geometry
```

### 74.2 Level-set interface evolution

```text
FUNCTION EvolveInterfaceLevelSet(phi0, speed_function F, curvature_weight gamma, T):
    phi = ReinitializeSignedDistance(phi0)
    FOR time steps:
        normal_speed = F(position, local_fields, parameters) + gamma * Curvature(phi)
        phi_t = -normal_speed * |grad(phi)|
        phi = UpwindHamiltonJacobiStep(phi, phi_t, dt)
        periodically reinitialize phi while preserving zero contour
    RETURN interface trajectory and numerical diagnostics
```

## 75. Unified mechanistic simulator

```text
FUNCTION SimulateMechanisticTissue(theta, project_geometry, simulator_spec):
    compiled = ValidateSimulator(simulator_spec)
    fields = InitializeFields(theta, project_geometry)
    agents = InitializeAgents(theta, project_geometry)

    FOR coupled interval:
        fields = AdvancePDEFields(fields, agents, theta)
        agents = AdvanceAgentProcess(agents, fields, theta)
        interfaces = AdvanceGrowthFront(interfaces, fields, agents, theta)
        vessels = AdvanceVascularModuleIfDynamic(vessels, fields, theta)
        CheckCouplingResidualsAndConservation()

    latent = {fields, agents, interfaces, vessels}
    observed = ApplyMicroscopyAndSegmentationObservationModel(latent, theta)
    RETURN SimulatedTissue(latent, observed, diagnostics)
```

## 76. Neural Cox processes — **FRONTIER / EXPERIMENTAL**

For domain `W`, point-process log likelihood is:

```text
log p(X|θ) = sum_{x_i in X} log λ_θ(x_i, context) - ∫_W λ_θ(u, context) du
```

### 76.1 Intensity network

```text
FUNCTION NeuralCoxIntensity(location, context, network):
    features = EncodeCoordinatesWithPhysicalScaleAndBoundaryFeatures(location)
    features += EncodeContext(context)
    raw = network(features)
    lambda = softplus(raw) + minimum_intensity
    RETURN lambda
```

### 76.2 Likelihood training

```text
FUNCTION TrainNeuralCoxProcess(patterns, windows, contexts, model, training_spec):
    FOR minibatch of independent patterns/patients:
        event_term = 0
        integral_term = 0

        FOR pattern p:
            FOR event x_i IN p:
                event_term += log NeuralCoxIntensity(x_i, context_p, model)

            quadrature = SampleOrFixedWindowQuadrature(window_p,
                                                       stratify_by_compartment=true)
            lambda_q = NeuralCoxIntensity(quadrature.locations, context_p, model)
            integral_term += WeightedQuadratureSum(lambda_q, quadrature.weights)

        loss = -(event_term - integral_term) + prior_or_regularization
        backpropagate and update

    validate integral estimator bias and variance
    calibrate counts and K/g posterior predictive behavior on heldout patients
    RETURN trained model with quadrature and domain provenance
```

### 76.3 Cox-process latent field variant

```text
latent_field z(x) = neural_field_or_GP(context, x)
log lambda(x) = offset(x) + z(x)
fit by VI/HMC/SBI depending field representation
```

## 77. Neural marked point processes — **FRONTIER / EXPERIMENTAL**

```text
FUNCTION NeuralMarkedPointLikelihood(pattern, window, context, model):
    // Factorization: location intensity times mark distribution conditional on location/history/context.
    location_loglik = sum_i log lambda_model(x_i, context)
                      - IntegralOverWindow(lambda_model)
    mark_loglik = 0
    FOR event i:
        mark_probs_or_density = mark_model(mark_i | x_i, spatial_context(pattern, i), context)
        mark_loglik += log mark_probs_or_density
    RETURN location_loglik + mark_loglik
```

For unordered static tissue patterns, do not impose an arbitrary event sequence. If an autoregressive model is used, it must marginalize or control ordering and pass permutation-invariance tests.

```text
FUNCTION TrainStaticNeuralMarkedProcess(patterns, model):
    use permutation-invariant set/graph encoders
    compute exact or controlled Monte Carlo integral
    optimize patient-level summed likelihood
    posterior/ensemble uncertainty required
    validate mark prevalence, spatial interaction, and multitype K/g
```

## 78. Normalizing-flow tissue generators — **FRONTIER / EXPERIMENTAL**

Variable-cardinality point sets require a count model plus a permutation-equivariant/invariant coordinate and mark model.

```text
FUNCTION FlowPointPatternModel(window, context):
    N ~ CountModel(context, window_area)
    base latent Z[1..N] ~ iid base distribution in canonical window coordinates
    X = EquivariantInvertibleFlow(Z, context, window_geometry)
    marks ~ ConditionalMarkFlowOrClassifier(X, context)
    RETURN X, marks
```

### 78.1 Flow training

```text
FUNCTION TrainPointSetFlow(patterns, model):
    FOR independent pattern p:
        n_loglik = log CountModelProbability(N_p | context_p)
        canonicalized_representation = PermutationInvariantFlowInterface(p)
        z, log_abs_det = InvertFlow(canonicalized_representation, context_p)
        coord_loglik = BaseLogDensity(z) + log_abs_det
        mark_loglik = ConditionalMarkLogLikelihood(p.marks | p.coordinates, context_p)
        loss += -(n_loglik + coord_loglik + mark_loglik)
    optimize
    test exact permutation invariance and boundary support
    RETURN model
```

If exact likelihood requires an arbitrary sort order, record the order model and do not claim set likelihood equivalence without proof.

## 79. Diffusion-based tissue generators — **FRONTIER / EXPERIMENTAL**

### 79.1 Fixed-cardinality point-set diffusion

```text
FOR diffusion time t:
    X_t = alpha_t X_0 + sigma_t epsilon
model predicts score s_theta(X_t, t, context) ≈ ∇_{X_t} log p_t(X_t|context)
```

```text
FUNCTION TrainPointSetDiffusion(patterns, score_model, noise_schedule):
    FOR minibatch:
        X0, masks, context = PadOrBucketByCardinality(patterns)
        t ~ TimeDistribution()
        epsilon ~ Normal(0,I) respecting masks
        Xt = alpha(t)*X0 + sigma(t)*epsilon
        predicted = score_model(SetEquivariantEncode(Xt, masks, context), t)
        target = -epsilon / sigma(t)
        loss = MaskedWeightedMSE(predicted, target)
        add boundary/support penalties or transform to unconstrained coordinates
        update parameters
    validate permutation equivariance and cardinality handling
```

### 79.2 Sampling

```text
FUNCTION SamplePointSetDiffusion(context, window, cardinality_model):
    N ~ cardinality_model(context)
    X_T ~ Normal(0,I) or base measure in transformed window
    FOR reverse times T -> 0:
        score = score_model(X_t, t, context)
        X_{t-dt} = ReverseSDEOrODEStep(X_t, score, schedule)
        project/transform to valid window support without introducing bias
    marks = conditional mark model
    RETURN generated pattern
```

## 80. Differentiable spatial summaries — **RESEARCH-ONLY**

```text
FUNCTION SoftPairHistogram(points, radii, bandwidth):
    FOR pair i<j:
        d = EuclideanDistance(points[i], points[j])
        FOR bin b near d:
            histogram[b] += SmoothKernel((d - center[b]) / bandwidth)
    normalize by declared intensity/window policy
    RETURN differentiable histogram
```

```text
FUNCTION SummaryMatchingLoss(generated, observed, summary_set):
    loss = 0
    FOR summary s:
        loss += weight_s * Distance(s(generated), s(observed))
    RETURN loss
```

Differentiable summaries supplement but do not replace likelihood or posterior-predictive validation.

## 81. Approximate Bayesian computation — **ESTABLISHED LIKELIHOOD-FREE FAMILY**

### 81.1 Rejection ABC

```text
FUNCTION RejectionABC(observed, prior, simulator, summary, distance, epsilon, N_accept):
    accepted = []
    attempts = 0
    WHILE accepted.length < N_accept AND attempts < max_attempts:
        theta ~ prior
        simulated = simulator(theta, Seed(ABC, attempts))
        d = distance(summary(simulated), summary(observed))
        IF d <= epsilon:
            accepted.append({theta, d, weight=1})
        attempts += 1

    IF accepted insufficient:
        RETURN InsufficientData("ABC acceptance too low")
    RETURN WeightedPosteriorSamples(accepted, acceptance_rate, epsilon)
```

### 81.2 SMC-ABC

```text
FUNCTION SMC_ABC(observed, prior, simulator, summary, distance,
                 epsilon_schedule, N_particles):
    particles = []

    FOR stage t IN 0..T-1:
        epsilon = epsilon_schedule[t] OR adaptive_quantile(previous_distances)
        new_particles = []

        WHILE new_particles.length < N_particles:
            IF t == 0:
                theta ~ prior
            ELSE:
                ancestor ~ Categorical(previous_weights)
                theta ~ perturbation_kernel(previous_particles[ancestor])
                IF prior(theta) == 0: CONTINUE

            simulated = simulator(theta, Seed(SMC_ABC,t,attempt))
            d = distance(summary(simulated), summary(observed))
            IF d > epsilon: CONTINUE

            IF t == 0:
                weight = 1
            ELSE:
                denominator = sum_j previous_weights[j]
                              * perturbation_density(theta | previous_theta[j])
                weight = prior_density(theta) / denominator

            new_particles.append({theta,d,weight})

        NormalizeWeights(new_particles)
        AdaptPerturbationKernel(new_particles)
        previous = new_particles

    RETURN posterior particles, epsilon history, ESS, simulation count
```

## 82. Synthetic likelihood — **ESTABLISHED LIKELIHOOD-FREE METHOD**

Assume summaries are approximately Gaussian under parameter `θ`:

```text
s(y) | θ ≈ Normal(mu_theta, Sigma_theta)
```

```text
FUNCTION EstimateSyntheticLogLikelihood(theta, observed_summary, simulator, R, shrinkage):
    summaries = []
    FOR r IN 1..R:
        y_r = simulator(theta, Seed(SYNTHETIC_LIKELIHOOD, r))
        summaries.append(summary(y_r))
    mu = Mean(summaries)
    Sigma = ShrinkageCovariance(summaries, shrinkage)
    IF Sigma not stable positive definite:
        RETURN Undefined("synthetic likelihood covariance unstable")
    loglik = MultivariateNormalLogDensity(observed_summary, mu, Sigma)
    uncertainty = MonteCarloLogLikelihoodSE(summaries)
    RETURN loglik, uncertainty
```

```text
FUNCTION SyntheticLikelihoodMCMC(observed, prior, simulator, proposal):
    initialize theta
    FOR iteration:
        theta_prop ~ proposal(theta)
        ll_prop = EstimateSyntheticLogLikelihood(theta_prop,...)
        ll_current = cached or unbiasedly refreshed according to algorithm
        accept with MH probability using prior, likelihood estimates, proposal ratio
        record Monte Carlo noise and failures
    RETURN chain and diagnostics
```

## 83. Neural posterior estimation — **FRONTIER / ESTABLISHED SBI TOOLCHAIN**

```text
FUNCTION TrainNPE(prior, simulator, context_encoder, conditional_density, rounds):
    proposal = prior
    dataset = []

    FOR round r:
        FOR simulation i:
            theta ~ proposal
            x = simulator(theta, Seed(NPE,r,i))
            context = context_encoder(x)
            dataset.append(theta, context,
                           importance_weight = prior(theta)/proposal(theta))

        train conditional_density q_phi(theta | context)
              by weighted maximum likelihood
        validate on heldout simulations
        proposal = DefensiveMixture(q_phi(theta | observed_context), prior)

    RETURN amortized posterior estimator with simulation provenance
```

## 84. Neural likelihood estimation — **FRONTIER / ESTABLISHED SBI TOOLCHAIN**

```text
FUNCTION TrainNLE(prior, simulator, likelihood_model, rounds):
    proposal = prior
    dataset = []
    FOR round:
        simulate pairs theta ~ proposal, x ~ simulator(theta)
        train q_phi(x | theta) by maximum likelihood
        IF sequential:
            proposal = MCMCPosteriorUsingLearnedLikelihood(observed, prior, q_phi)
    posterior_samples = MCMC_or_SMC(target ∝ prior(theta)*q_phi(observed|theta))
    RETURN learned likelihood, posterior samples, diagnostics
```

## 85. Neural ratio estimation — **FRONTIER / ESTABLISHED SBI TOOLCHAIN**

Train classifier to distinguish joint samples `p(θ,x)` from independent samples `p(θ)p(x)`.

```text
FUNCTION TrainNRE(prior, simulator, ratio_classifier, rounds):
    proposal = prior
    FOR round:
        joint = [(theta, simulator(theta)) for theta ~ proposal]
        marginal_x = PermuteXAcrossTheta(joint) or simulate independently
        train classifier d_phi(theta,x) with labels joint=1, independent=0
        log_ratio(theta,x) = logit(d_phi(theta,x))
        proposal = SamplePosterior(prior(theta)*exp(log_ratio(theta, observed)))
    RETURN ratio estimator and posterior sampler
```

## 86. Sequential SBI controller

```text
FUNCTION SequentialSBI(observed, method, prior, simulator, budget, diagnostics):
    proposal = prior
    all_rounds = []

    FOR round r UNTIL budget exhausted:
        simulations = SimulateFromProposal(proposal, allocated_budget[r])
        fit = TrainOrUpdateSBIModel(method, simulations, previous_models)

        calibration = HeldoutSimulationDiagnostics(fit)
        posterior = InferPosterior(fit, observed, prior)
        support_check = CheckPosteriorWithinPriorAndSimulationSupport(posterior, simulations)

        all_rounds.append({simulations, fit, calibration, support_check})

        IF support_check fails:
            proposal = BroadenWithPriorMixture(posterior)
        ELSE:
            proposal = DefensivePosteriorProposal(posterior)

        IF ConvergedAndCalibrated(all_rounds): BREAK

    RETURN posterior, round history, diagnostics, simulation ledger
```

## 87. Simulation-based calibration — **MANDATORY FOR BAYES/SBI RELEASE**

```text
FUNCTION SimulationBasedCalibration(prior, simulator, inference_algorithm, R):
    ranks_by_parameter = []
    failures = []

    FOR r IN 1..R:
        theta_true ~ prior
        y ~ simulator(theta_true, Seed(SBC,r))
        fit = inference_algorithm(y)

        IF fit failed or nonconverged:
            failures.append(r, reason)
            CONTINUE

        posterior_draws = fit.draws
        FOR scalar test_quantity q:
            rank = Count(draw_q < q(theta_true))
                   + RandomizedTieAdjustment()
            ranks_by_parameter[q].append(rank)

    FOR q:
        assess discrete uniformity with simultaneous bands
        inspect rank histograms for bias, under/overdispersion, and autocorrelation
    report failure rate separately; never drop failures silently
    RETURN SBC report
```

## 88. OOD detection for SBI and generative models

```text
FUNCTION DetectSimulationOOD(observed, simulation_bank, encoder, method):
    z_obs = encoder(observed)
    z_sim = encoder(simulation_bank)

    SWITCH method:
        KNN_DISTANCE:
            score = distance to k-nearest simulated summaries
        DENSITY:
            score = -log density_model(z_obs)
        CLASSIFIER:
            train classifier observed/reference-like vs simulated using heldout controls
        CONFORMAL:
            score and threshold from calibration simulations

    compare score with simulation-calibrated threshold
    RETURN in_support / out_of_support / indeterminate plus diagnostics
```

## 89. Posterior-predictive tissue laboratory

```text
FUNCTION PosteriorPredictiveLaboratory(posterior, simulator, observed, summary_catalog, M):
    checks = []
    FOR m IN 1..M:
        theta_m ~ posterior
        y_rep_m = simulator(theta_m, Seed(PPC,m))
        FOR summary s IN summary_catalog:
            checks[s].append(s(y_rep_m))

    observed_summaries = {s: s(observed)}
    FOR s:
        compute posterior predictive intervals and discrepancy p-values
        compare K/L/g, mark functions, counts, compartments, graph, topology,
                embeddings, interfaces, and multimodal cross-statistics

    RETURN check dashboard, failures, and model-misspecification flags
```

## 90. Generative-model reliability tests

```text
FUNCTION ValidateGenerativeTissueModel(model, training_data, heldout_data):
    test cardinality and window support
    test intensity, K/L/g, mark and cross-type functions
    test graph, motif, topology, compartment, and interface summaries
    test nearest-neighbor and rare-event behavior
    test mode coverage across patients and sites
    run nearest-neighbor memorization and membership-inference audits
    test conditional generation consistency
    test seed reproducibility and stochastic diversity
    compare against simpler Poisson/LGCP/Gibbs/mechanistic baselines
    reject promotion if simpler models explain data equally well
    RETURN model card and maturity decision
```

---
# Part XI — 3-D, Longitudinal, and Evolutionary Models

## 91. Dimensionality and anisotropy contract

No 2-D estimator may silently accept 3-D coordinates.

```text
TYPE SpatialDimensionSpec:
    dimension                  // 2 or 3
    axis_names
    axis_units
    voxel_or_pixel_spacing
    anisotropy_matrix OPTIONAL
    z_section_thickness_um OPTIONAL
    z_gap_um OPTIONAL
    coordinate_frame
    metric                     // Euclidean, anisotropic Mahalanobis, geodesic
    boundary_representation

FUNCTION ValidateDimensionality(data, spec):
    REQUIRE coordinate column count == spec.dimension
    REQUIRE all units convertible to micrometres
    REQUIRE positive finite spacings
    reject use of 2-D correction formulas for 3-D data
    RETURN normalized coordinates and metric
```

## 92. Serial-section reconstruction with uncertain deformation — **ADVANCED / FRONTIER**

### 92.1 Pairwise-to-stack registration

```text
FUNCTION ReconstructSerialSectionStack(sections, landmarks_or_images, reconstruction_spec):
    reference = SelectReferenceSection(sections, reconstruction_spec)
    pairwise_models = []

    FOR adjacent or reference-connected section pair (a,b):
        transform_ab = FitRigidAffineThenDiffeomorphic(
                           source=section[a], target=section[b],
                           masks, landmarks, image_features,
                           regularization=reconstruction_spec)
        uncertainty_ab = EstimateTransformUncertainty(transform_ab,
                                                       residuals,
                                                       bootstrap_or_posterior)
        pairwise_models.append(transform_ab, uncertainty_ab)

    global_transforms = SolvePoseGraphOrJointRegistration(pairwise_models,
                                                          anchor=reference,
                                                          cycle_consistency_penalty)
    stack_coordinates = ApplyTransformsToAllSectionObjects(global_transforms)

    z_coordinates = IntegrateSectionThicknessAndMissingGaps(sections.metadata)
    validate nonfolding, cycle consistency, landmark residuals, and tissue overlap
    RETURN 3DStack(global_transforms, uncertainty, z_coordinates, diagnostics)
```

### 92.2 Joint Bayesian stack reconstruction

```text
FUNCTION FitBayesianSectionStack(sections, model_spec):
    FOR section s:
        rigid parameters r_s ~ prior around acquisition ordering
        velocity field v_s ~ smooth GP/GMRF prior
        transform phi_s = ExpVelocity(v_s) ∘ Rigid(r_s)

    likelihood includes:
        landmark residuals
        image similarity or feature correspondence
        boundary/compartment consistency
        optional cell-density consistency

    add penalties/priors for:
        cycle consistency
        adjacent-section smoothness
        Jacobian positivity
        missing-section interpolation

    posterior = fit with HMC for low-dimensional globals + VI/Laplace for fields,
                or external validated registration backend with uncertainty artifact
    RETURN posterior transforms and sampled reconstructed stacks
```

## 93. Propagating stack uncertainty

```text
FUNCTION PropagateStackUncertainty(stack_posterior, downstream_analysis, M):
    outputs = []
    FOR m IN 1..M:
        transforms_m ~ stack_posterior
        stack_m = ApplyTransforms(sections, transforms_m)
        outputs.append(downstream_analysis(stack_m))
    RETURN posterior/multiple-imputation summary and between-transform variance
```

## 94. 3-D observation windows

```text
TYPE ObservationWindow3D:
    representation             // tetrahedral mesh, watertight surface mesh, voxel mask
    coordinate_frame
    volume_um3
    surface_area_um2
    components
    cavities
    containment_index
    boundary_distance_index
    provenance

FUNCTION ValidateWindow3D(window):
    verify watertight/oriented surface or valid voxel/tetrahedral representation
    verify positive finite volume
    verify no invalid self-intersection where exact mesh is required
    verify coordinate scale and z anisotropy
    RETURN compiled 3-D window
```

## 95. 3-D point-process statistics — **ESTABLISHED EXTENSION WITH NEW ORACLES**

For homogeneous isotropic Poisson in 3-D, `K(r) = 4πr³/3`; define `L_3(r) = (3K(r)/(4π))^(1/3)`.

```text
FUNCTION KFunction3D(points, window3d, radii, correction):
    n = number of points
    lambda_hat = n / window3d.volume
    pair_plan = Build3DPairPlan(points, max(radii), anisotropic_metric)

    FOR radius r:
        numerator = 0
        denominator = 0
        FOR ordered/undirected eligible pair (i,j,d) with d <= r:
            edge_weight = EdgeCorrection3D(i,j,window3d,correction)
            numerator += edge_weight
        K[r] = Normalize3DK(numerator, n, lambda_hat, convention)
        L[r] = (3*K[r]/(4*pi))^(1/3)
    RETURN curves with correction and exact/approx mode
```

Possible corrections include border, translation-volume overlap, or isotropic surface fraction; each requires separate numerical fixtures.

## 96. 3-D inhomogeneous and multitype processes

```text
FUNCTION InhomogeneousK3D(points, intensity_values, window, radii, correction):
    FOR pair i != j:
        contribution = EdgeWeight3D(i,j) / (lambda_i * lambda_j)
        accumulate by distance bin/radius
    normalize by window volume and convention
    RETURN Kinhom_3D

FUNCTION CrossK3D(type_a_points, type_b_points, ...):
    directed pair accumulation with type-specific intensities
    RETURN directed cross-K and cross-g curves
```

## 97. 3-D fields and GPs

```text
FUNCTION FitAnisotropic3DGP(observations, coordinates_xyz, anisotropy_prior, backend):
    transform metric using positive-definite anisotropy matrix A
    distance h = sqrt((x_i-x_j)^T A (x_i-x_j))
    covariance = Matern(h; sigma, range, nu)
    fit exact, inducing, NNGP, or SPDE posterior
    report axis-specific effective ranges and identifiability
    RETURN posterior field and diagnostics
```

## 98. 3-D graphs and higher-order structures

```text
FUNCTION Build3DSpatialGraph(objects, graph_spec):
    use 3-D spatial index and physical radius/kNN
    respect anisotropic metric and registration uncertainty
    build sparse operator and graph digest
    RETURN graph

FUNCTION Build3DAlphaComplex(points, max_alpha):
    construct 3-D Delaunay tetrahedralization
    include vertices, edges, triangles, tetrahedra under alpha criterion
    run persistent homology through dimension 2 or 3 as specified
    RETURN filtration
```

## 99. Spatiotemporal state-space models — **ESTABLISHED / ADVANCED**

General model:

```text
latent state x_t = F_t(x_{t-1}, covariates_t) + process_noise
observation y_t ~ p(y_t | H_t(x_t), measurement_noise)
```

### 99.1 Linear-Gaussian Kalman filter/smoother

```text
FUNCTION KalmanFilter(y[1:T], F, Q, H, R, m0, P0):
    m = m0; P = P0
    FOR t IN 1..T:
        m_pred = F_t m
        P_pred = F_t P F_t^T + Q_t

        innovation = y_t - H_t m_pred
        S = H_t P_pred H_t^T + R_t
        K = P_pred H_t^T S^{-1}

        m = m_pred + K innovation
        P = JosephStableCovarianceUpdate(P_pred, K, H_t, R_t)
        store filtered and predicted states

    log_likelihood = SumInnovationLogLikelihoods()
    RETURN filtered states and log_likelihood

FUNCTION RauchTungStriebelSmoother(filter_output):
    initialize smoothed_T = filtered_T
    FOR t FROM T-1 DOWNTO 1:
        J_t = P_filtered_t F_{t+1}^T P_pred_{t+1}^{-1}
        m_smooth_t = m_filtered_t + J_t(m_smooth_{t+1}-m_pred_{t+1})
        P_smooth_t = P_filtered_t + J_t(P_smooth_{t+1}-P_pred_{t+1})J_t^T
    RETURN smoothed states
```

### 99.2 Extended/unscented filtering

```text
FUNCTION NonlinearGaussianFilter(y, transition f, observation h, method):
    IF method == EKF:
        linearize f and h by Jacobians at current mean
        apply Kalman equations
    ELSE IF method == UKF:
        generate sigma points
        propagate through f and h
        reconstruct moments and update
    RETURN approximate filter with linearization/sigma-point diagnostics
```

### 99.3 Particle filter and smoother

```text
FUNCTION ParticleFilter(y[1:T], transition, observation, N_particles):
    particles ~ initial_distribution
    weights = uniform

    FOR t IN 1..T:
        FOR p:
            proposed[p] ~ transition(particles[p], covariates_t)
            log_weight[p] = log observation_density(y_t | proposed[p])
        normalize weights stably
        ESS = 1 / sum(weights^2)
        IF ESS < threshold:
            ancestors = SystematicResample(weights)
            particles = proposed[ancestors]
            weights = uniform
        ELSE:
            particles = proposed
        store particles, weights, ancestry

    RETURN filtering distribution, marginal likelihood estimate, ancestry
```

```text
FUNCTION ParticleSmoother(filter_output, M_trajectories):
    sample terminal particles by terminal weights
    trace ancestors backward or use backward simulation
    RETURN smoothed trajectories
```

### 99.4 Spatial latent-state model

```text
x_t spatial field ~ DynamicGMRF/GP(F x_{t-1}, Q)
y_t at cells/regions ~ modality-specific likelihood(H x_t)
fit using Kalman methods for Gaussian models, particle methods, Laplace, or VI
```

## 100. Deformation-versus-biological-change separation — **FRONTIER / IDENTIFIABILITY-LIMITED**

Observed follow-up tissue may differ because of deformation, sampling, and true biological change.

```text
FUNCTION FitDeformationBiologyModel(pre, post, model_spec):
    latent baseline field B(x)
    deformation phi with smooth diffeomorphic prior
    biological change Delta(x) with sparse/smooth/domain prior

    post observation model:
        Y_post(x) ~ ObservationModel(B(phi^{-1}(x)) + Delta(x), noise)
    pre observation model:
        Y_pre(x) ~ ObservationModel(B(x), noise)

    include landmarks, anatomy, and negative-control features to identify phi
    include independent molecular/IHC evidence to inform Delta when available
    fit joint posterior

    run identifiability diagnostics:
        posterior correlation between deformation and Delta
        recovery under known simulated deformation/change
        prior sensitivity
        null controls with deformation only and change only

    IF separation not identifiable:
        RETURN PartialIdentification(bounds_or_sensitivity_set)
    RETURN posterior deformation and biological-change fields
```

## 101. Clone phylogeography — **ADVANCED; CROSS-SECTIONAL LIMITS EXPLICIT**

### 101.1 Tree-conditioned spatial diffusion model

```text
FUNCTION FitClonePhylogeography(tree, clone_locations, uncertainty, model_spec):
    REQUIRE imported phylogeny/clone assignments with provenance
    FOR branch parent -> child with length dt:
        location_child ~ SpatialTransition(location_parent, dt, diffusion_or_drift_parameters)

    observation:
        clone cell/region coordinates ~ distribution around latent clone location/territory
        incorporate assignment uncertainty and sampling window

    infer ancestral locations, diffusion parameters, and uncertainty
    validate against simulated trees and spatial sampling
    RETURN descriptive phylogeographic posterior
```

Cross-sectional tissue cannot identify historical migration paths uniquely. Report plausible ancestral locations or isolation-by-distance structure, not literal observed migration.

### 101.2 Phylogenetic versus spatial-distance association

```text
FUNCTION PhylogeneticSpatialAssociation(clones, tree, spatial_summary, design):
    D_phylo = PairwiseTreeDistance(tree, clone_ids)
    D_spatial = PairwiseCloneSpatialDistanceOrTerritoryDistance(clones)

    statistic = DistanceCovarianceOrMantelLikeStatistic(D_phylo, D_spatial)
    null = PermuteCloneLabelsWithinPermittedPatient/SpecimenBlocks(design)
    p_value = RestrictedPermutation(statistic, null)
    RETURN association with noncausal/cross-sectional limitation
```

### 101.3 Clone-specific niche model

```text
FUNCTION FitCloneNicheModel(cells, clone_probabilities, neighborhood_features, hierarchy):
    latent clone assignment integrated over imported uncertainty
    neighborhood composition ~ hierarchical model by clone with patient random effects
    optional spatial GP/GMRF residual
    patient-level posterior contrasts among clones
    RETURN clone-niche effects and uncertainty
```

## 102. 3-D/longitudinal validation suite

```text
FUNCTION Validate3DLongitudinalSuite():
    simulate known rigid/nonrigid serial stacks with missing sections
    evaluate transform recovery, cycle consistency, and coverage
    verify 3-D K/L against Poisson and hand-volume fixtures
    verify anisotropic GP range recovery
    verify 3-D graph and alpha-complex outputs against trusted references
    simulate linear-Gaussian state models and test Kalman coverage
    simulate nonlinear/non-Gaussian trajectories and test particle calibration
    simulate deformation-only, change-only, and mixed scenarios
    test phylogeographic recovery only under identifiable simulated models
    report failures and cross-sectional nonidentifiability explicitly
```

---
# Part XII — Causal, Interference, Perturbational, and Active-Design Research

These methods are valid only when treatment/exposure, assignment mechanism, timing, eligible units, outcomes, and causal assumptions are explicitly defined. Ordinary cross-sectional proximity, co-expression, attention, or prediction is not causal evidence.

## 103. Causal design contract

```text
TYPE CausalDesign:
    unit_id
    cluster_id                  // patient, specimen, field, well, animal, etc.
    treatment Z
    treatment_time
    outcome Y
    outcome_time
    baseline_covariates X
    spatial_coordinates
    adjacency_or_distance
    eligibility
    assignment_mechanism       // randomized, observational, unknown
    interference_scope
    exposure_mapping_spec
    censoring
    missingness
    negative_controls
    estimand_spec
    provenance

FUNCTION ValidateCausalDesign(design):
    require temporal ordering of treatment before outcome
    require stable unit and cluster identity
    verify treatment variation and eligible comparison support
    verify no post-treatment covariates in baseline adjustment
    validate interference graph/exposure map is prespecified
    assess positivity/overlap
    identify whether design is randomized, quasi-experimental, or observational
    IF assignment mechanism unknown and no defensible identification strategy:
        RETURN UnsupportedForClaim("causal effect not identified")
    RETURN compiled causal design and explicit assumptions
```

## 104. Potential outcomes with spatial interference — **ESTABLISHED FRAMEWORK; SPECIALIZED APPLICATION**

Let potential outcome `Y_i(z_i, g_i(z_-i))` depend on own treatment and an exposure mapping of neighbours’ treatments.

### 104.1 Exposure mapping

```text
FUNCTION ComputeExposureMapping(i, treatment_vector Z, graph, spec):
    neighbors = graph.neighbors(i)
    SWITCH spec.kind:
        BINARY_ANY_TREATED:
            g = any(Z_j == 1 for j in neighbors)
        COUNT:
            g = sum_j Z_j
        FRACTION:
            g = weighted_sum_j w_ij Z_j / weighted_sum_j w_ij
        DISTANCE_DECAY:
            g = sum_j kernel(distance_ij, spec.bandwidth_um) * Z_j
        MULTISCALE:
            g = vector over physical radii
        CONTINUOUS_FIELD:
            g = declared spatial treatment field evaluated at i
    RETURN g
```

Exposure mapping must be specified before outcome inspection for confirmatory use.

### 104.2 Horvitz–Thompson/Hájek exposure estimators under known randomization

```text
FUNCTION EstimateExposureMean(outcomes Y, observed_exposures E,
                              target_exposure e, exposure_probabilities pi):
    eligible = {i : pi_i(e) > 0}
    IF eligible insufficient:
        RETURN InsufficientData("no positivity for target exposure")

    HT_total = sum_i I(E_i=e) * Y_i / pi_i(e)
    HT_mean = HT_total / number_of_eligible_units

    Hajek_num = sum_i I(E_i=e) * Y_i / pi_i(e)
    Hajek_den = sum_i I(E_i=e) / pi_i(e)
    Hajek_mean = Hajek_num / Hajek_den

    variance = RandomizationVarianceEstimatorOrClusterBootstrap(...)
    RETURN means and uncertainty
```

```text
FUNCTION DirectAndSpilloverEffects(exposure_means):
    direct_effect(g) = mean(Y | own=1, exposure=g) - mean(Y | own=0, exposure=g)
    spillover_effect(z, g1, g0) = mean(Y | own=z, exposure=g1)
                                    - mean(Y | own=z, exposure=g0)
    total_effect = selected joint exposure contrast
    RETURN prespecified contrasts
```

### 104.3 Randomization inference

```text
FUNCTION InterferenceRandomizationTest(design, statistic, B):
    observed = statistic(design.Y, design.Z, ComputeAllExposures(design.Z))
    null_values = []

    FOR b IN 0..B-1:
        Z_b = SampleFromActualAssignmentMechanism(design, Seed(INTERFERENCE,b))
        E_b = ComputeAllExposures(Z_b)
        null_values.append(statistic(design.Y, Z_b, E_b))

    p_value = InclusivePermutationPValue(observed, null_values, alternative)
    RETURN p_value and null distribution
```

## 105. Continuous spatial treatments and dose–response

```text
FUNCTION FitSpatialDoseResponse(design, treatment A, exposure G, model_spec):
    estimate generalized propensity density r(a,g | X, spatial_context)
    verify joint support across target (a,g) grid

    outcome_model = FitFlexibleOutcomeModel(Y ~ A + G + A×G + X + spatial_random_effect,
                                            cross_fitting_by_cluster)

    FOR target (a,g):
        mu(a,g) = average_i PredictOutcome(outcome_model,
                                           A=a, G=g, X=X_i,
                                           integrate_random_effects_as_declared)
    uncertainty = cluster bootstrap or Bayesian posterior
    RETURN dose-response surface with unsupported regions masked
```

## 106. Spatial propensity scores — **ADVANCED; DESIGN-SENSITIVE**

```text
FUNCTION EstimateSpatialPropensity(design, treatment_type, learner_spec):
    features = baseline covariates
             + prespecified location/compartment/site terms
             + pre-treatment spatial-context summaries

    split by independent cluster/patient
    FOR cross-fit fold:
        train treatment model on training clusters only
        IF binary:
            e_i = P(Z_i=1 | features_i)
        ELSE IF categorical:
            e_i[k] = P(Z_i=k | features_i)
        ELSE:
            r_i(a) = conditional treatment density
        predict heldout fold

    clip only under prespecified policy; record affected fraction
    assess overlap globally and by site/compartment
    RETURN cross-fitted propensity estimates and diagnostics
```

Spatial coordinates must not be used as an unrestricted identifier that destroys overlap or memorizes site/patient.

## 107. Doubly robust estimation — **ESTABLISHED**

### 107.1 Binary treatment AIPW without interference

```text
FUNCTION CrossFittedAIPW(design, outcome_learner, propensity_learner, K_folds):
    folds = SplitByIndependentCluster(design.cluster_id, K_folds)
    influence = zeros(n)

    FOR fold k:
        train = not fold k; test = fold k
        e_hat = FitPropensity(train).predict(test.X)
        m1_hat = FitOutcome(train with Z=1).predict(test.X)
        m0_hat = FitOutcome(train with Z=0).predict(test.X)

        FOR i IN test:
            influence[i] = (m1_hat[i] - m0_hat[i])
                         + Z_i*(Y_i-m1_hat[i])/e_hat[i]
                         - (1-Z_i)*(Y_i-m0_hat[i])/(1-e_hat[i])

    ate = mean(influence)
    se = ClusterRobustSE(influence, cluster_id)
    RETURN ate, se, CI, nuisance diagnostics
```

### 107.2 Doubly robust exposure effect under interference

```text
FUNCTION ExposureAIPW(design, target_exposures e1,e0, nuisance_models):
    cross-fit by randomized/independent clusters
    estimate:
        pi_i(e) = P(E_i=e | X_i, assignment/design)
        m_i(e)  = E[Y_i | E_i=e, X_i]

    phi_i(e) = m_i(e) + I(E_i=e)/pi_i(e) * (Y_i - m_i(e))
    effect = mean_i[phi_i(e1) - phi_i(e0)]
    variance = cluster-robust influence-function variance
    RETURN effect with positivity and model diagnostics
```

## 108. Double/debiased machine learning — **ESTABLISHED GENERAL FRAMEWORK**

```text
FUNCTION SpatialDML(design, treatment, outcome, nuisance_learners, score_spec):
    folds = SplitByPatientSiteOrRandomizationCluster()
    scores = []

    FOR fold:
        train nuisance functions on other folds:
            m_hat(X,S) = E[Y | X,S]
            g_hat(X,S) = E[D | X,S]
        heldout residuals:
            Y_tilde = Y - m_hat
            D_tilde = D - g_hat

        IF partially linear score:
            psi_i(theta) = (Y_tilde_i - theta D_tilde_i) * D_tilde_i
        ELSE:
            use prespecified orthogonal score for treatment/exposure estimand

        solve mean psi(theta)=0 on heldout observations
        retain influence scores

    aggregate folds
    compute cluster-robust uncertainty
    test nuisance stability and overlap
    RETURN causal estimate only if identification assumptions pass
```

## 109. Negative controls — **ESTABLISHED DIAGNOSTIC STRATEGY**

```text
FUNCTION NegativeControlAnalysis(design, negative_control_exposure, negative_control_outcome):
    validate proposed negative control is plausibly unaffected/unrelated under causal model

    nc_exposure_result = FitPrimaryEstimator(
        treatment=negative_control_exposure,
        outcome=primary_outcome,
        same_adjustment_set)

    nc_outcome_result = FitPrimaryEstimator(
        treatment=primary_treatment,
        outcome=negative_control_outcome,
        same_adjustment_set)

    IF either indicates unexplained association beyond calibrated threshold:
        flag residual confounding/measurement bias
    RETURN diagnostic; do not mechanically “correct” without an identified model
```

## 110. Sensitivity analysis for unmeasured confounding

### 110.1 Rosenbaum-style hidden-bias sensitivity for matched designs

```text
FUNCTION RosenbaumSensitivity(matched_sets, outcomes, treatments, Gamma_grid):
    FOR Gamma IN Gamma_grid:
        bound treatment odds ratio within matched set by Gamma
        compute worst-case upper and lower randomization p-value bounds
    critical_Gamma = largest Gamma retaining conclusion
    RETURN sensitivity curve and critical_Gamma
```

### 110.2 Bias-function sensitivity

```text
FUNCTION BiasFunctionSensitivity(observed_effect, parameters_grid):
    FOR assumed confounder prevalence/effect parameters:
        bias = ComputeBiasUnderSensitivityModel(parameters)
        adjusted_effect = observed_effect - bias
    RETURN identified/sensitivity region
```

## 111. Partial identification — **ESTABLISHED PRINCIPLE; MODEL-SPECIFIC IMPLEMENTATION**

```text
FUNCTION PartialIdentificationBounds(observed_data, assumptions):
    define feasible set of latent/counterfactual distributions consistent with:
        observed-data constraints
        treatment assignment constraints
        monotonicity/exclusion/bounded-interference assumptions if declared

    lower = OptimizeEstimand(feasible_set, minimize)
    upper = OptimizeEstimand(feasible_set, maximize)
    IF sampling uncertainty needed:
        bootstrap or invert tests at cluster level
    RETURN [lower, upper] with exact assumptions
```

Do not replace a wide valid bound with a point estimate merely because it is uninformative.

## 112. Treatment-spillover and perturbational workflow

```text
FUNCTION AnalyzeSpatialPerturbationExperiment(project, design_spec):
    design = ValidateCausalDesign(project.perturbation_data)
    graph = BuildPreTreatmentInterferenceGraphOrDistanceOperator()
    exposures = ComputeMultiscaleExposureMappings(design.Z, graph, design_spec)

    primary = EstimatePrespecifiedDirectAndSpilloverEffects(
                  design.Y, design.Z, exposures,
                  assignment_mechanism=design.assignment_mechanism)

    run randomization inference if randomized
    run DR/DML estimators as secondary robustness analyses
    run negative controls and overlap diagnostics
    correct multiplicity across prespecified scales/outcomes
    perform patient/animal/well-level replication inference
    RETURN causal result only for identified contrasts; others association-only
```

## 113. Mediation through spatial neighborhoods — **RESEARCH-ONLY**

```text
FUNCTION SpatialMediationAnalysis(treatment, mediator_neighborhood, outcome, design):
    require temporal ordering and defensible no-unmeasured-confounding assumptions
    explicitly model treatment-induced mediator and interference
    estimate interventional/direct/indirect effects under declared exposure mapping
    run sensitivity analysis for mediator-outcome confounding
    IF assumptions unsupported:
        return association decomposition, not causal mediation
```

## 114. Bayesian experimental design — **ESTABLISHED FRAMEWORK; ADVANCED IMPLEMENTATION**

For candidate design `d`, expected information gain is:

```text
EIG(d) = E_{theta~p(theta), y~p(y|theta,d)}[
             log p(theta|y,d) - log p(theta)
         ]
```

### 114.1 Nested Monte Carlo EIG

```text
FUNCTION EstimateExpectedInformationGain(candidate d, prior, simulator, likelihood, N_outer, N_inner):
    values = []
    FOR o IN 1..N_outer:
        theta_o ~ prior
        y_o ~ simulator(theta_o, d, Seed(EIG_OUTER,o))
        log_num = log likelihood(y_o | theta_o, d)

        theta_inner[1..N_inner] ~ prior
        log_den = LogMeanExp_j log likelihood(y_o | theta_inner[j], d)
        values.append(log_num - log_den)

    estimate = mean(values)
    se = MonteCarloSE(values)
    RETURN estimate, se, bias diagnostics
```

### 114.2 Posterior-conditioned sequential design

```text
FUNCTION SequentialBayesianDesign(current_posterior, candidate_set, budget, utility):
    WHILE budget remains:
        FOR candidate d IN feasible candidates:
            utility_estimate[d] = EstimatePosteriorExpectedUtility(
                                      current_posterior, d, utility)
        d_star = SelectCandidateWithExplorationAndUncertainty(utility_estimate)
        acquire observation y_star under d_star
        current_posterior = UpdatePosterior(current_posterior, y_star, d_star)
        remove/modify candidates and budget
        record decision provenance and realized utility
    RETURN acquisition sequence and posterior history
```

## 115. Active ROI and field-of-view selection

```text
FUNCTION SelectActiveROIs(slide, posterior_or_model, candidate_rois, budget, objective):
    FOR roi IN candidate_rois:
        quality = PredictTissueFocusCoverageAndArtifactRisk(roi)
        cost = AcquisitionCost(roi)
        diversity = DistanceFromAlreadySelectedROIs(roi)

        SWITCH objective:
            PARAMETER_INFORMATION:
                utility = EIG(roi)
            PREDICTIVE_UNCERTAINTY:
                utility = ExpectedReductionInHeldoutPredictiveEntropy(roi)
            DOMAIN_COVERAGE:
                utility = CoverageGainInEmbeddingOrSpatialFingerprintSpace(roi)
            RARE_EVENT:
                utility = ProbabilityOfCapturingTargetEvent(roi)

        score[roi] = utility + diversity_weight*diversity - cost_penalty*cost
        score[roi] *= quality

    selected = BudgetedSubmodularOrIntegerOptimization(score,
                                                        overlap_constraints,
                                                        compartment_quotas,
                                                        patient_balance)
    RETURN selected ROIs and expected utility with uncertainty
```

## 116. Active stain selection

```text
FUNCTION SelectAdditionalStains(current_modalities, candidate_stains, posterior, budget):
    FOR stain m:
        predictive_distribution = PosteriorPredictUnobservedStain(m)
        utility[m] = ExpectedInformationGainAboutTargetParametersOrDecision(
                         predictive_distribution, posterior)
        redundancy[m] = ConditionalMutualInformationWithCurrentModalities()
        robustness[m] = ExpectedTechnicalSuccessAndBatchSensitivity()
        net[m] = utility[m] - redundancy_penalty*redundancy[m]
                 - cost[m] - failure_penalty*(1-robustness[m])

    solve budgeted selection with panel compatibility constraints
    RETURN recommended stains, expected gains, and assumption-sensitive alternatives
```

## 117. Active landmark allocation for registration

```text
FUNCTION SelectRegistrationLandmarks(source, target, current_transform_posterior,
                                     candidate_landmarks, budget):
    FOR candidate c:
        predicted_correspondence_uncertainty = EstimateMatchUncertainty(c)
        fisher_information_gain = ExpectedTransformParameterInformation(c,
                                                                         current_posterior)
        spatial_coverage_gain = CoverageGain(c, selected_landmarks)
        deformation_hotspot_gain = ExpectedReductionInLocalWarpUncertainty(c)
        utility[c] = weighted sum - annotation_cost(c)

    selected = GreedyOrOptimalDesign(utility,
                                     minimum_separation,
                                     compartment_coverage,
                                     boundary_coverage,
                                     budget)
    RETURN landmark plan and expected registration-uncertainty reduction
```

## 118. Replicate and region allocation

```text
FUNCTION AllocateReplicates(total_budget, design_options, pilot_model, target_estimand):
    FOR feasible allocation a across patients/specimens/slides/regions/cells:
        simulate datasets under posterior/pilot uncertainty
        estimate power, interval width, or decision utility for target estimand
        penalize technical-only replication with low biological information gain
        utility[a] = expected scientific utility - acquisition cost
    choose robust allocation under parameter uncertainty
    RETURN recommended allocation and sensitivity frontier
```

## 119. Prospective power for spatial endpoints

```text
FUNCTION SpatialPowerSimulation(design_grid, generative_models, estimator, alpha, R):
    FOR design d:
        FOR scenario theta:
            rejections = 0; failures = 0
            FOR r IN 1..R:
                data = SimulateFullCohortSpatialStudy(d, theta, Seed(POWER,d,theta,r))
                fit = estimator(data)
                IF fit failed:
                    failures += 1
                ELSE IF fit.primary_p <= alpha:
                    rejections += 1
            power[d,theta] = rejections / R
            failure_rate[d,theta] = failures / R
            interval = WilsonInterval(rejections, R)
    RETURN power surfaces and robust design recommendations
```

## 120. Causal and active-design validation suite

```text
FUNCTION ValidateCausalActiveSuite():
    simulate randomized clusters with known direct/spillover effects
    verify exposure probability and HT/Hájek unbiasedness
    test DR estimator under one correct and one misspecified nuisance model
    test DML coverage with cluster cross-fitting
    demonstrate positivity failure detection
    simulate unmeasured confounding and verify sensitivity/partial-ID behavior
    validate negative controls under null and confounded scenarios
    compare EIG estimators with analytically tractable Gaussian designs
    validate active selection against random and heuristic baselines
    require external prospective validation before operational laboratory claims
```

---
# Part XIII — Unified Execution, Validation, and Release Contracts

## 121. Canonical algorithm descriptor

```text
TYPE AlgorithmDescriptor:
    algorithm_id
    canonical_name
    version
    maturity ∈ {validated, established, experimental, research_only,
                unsupported_for_claim}
    scientific_role ∈ {descriptive, inferential, predictive, generative,
                       causal, design}
    dimensionality
    input_schemas[]
    output_schema
    estimand_or_target
    assumptions[]
    permitted_replication_units[]
    permitted_randomization_units[]
    exact_modes[]
    approximate_modes[]
    backend_requirements[]
    numerical_oracles[]
    calibration_suite
    performance_budget
    claim_limits[]
    provenance
```

## 122. Unified algorithm invocation

```text
FUNCTION ExecuteAlgorithm(project, algorithm_descriptor, configuration, input_refs):
    run_context = BeginReproducibleRun(project,
                                       algorithm_descriptor,
                                       configuration,
                                       input_refs)

    // 1. Contract validation
    validated_inputs = ValidateInputSchemas(input_refs,
                                            algorithm_descriptor.input_schemas)
    design = ResolveCohortAndInferenceDesign(project, configuration)
    ValidateReplicationAndRandomization(design, algorithm_descriptor)
    ValidateCoordinateFramesAndUnits(validated_inputs)
    ValidateMeasurementVersusPredictionStatus(validated_inputs)

    // 2. Capability and support checks
    mode = SelectExecutionMode(configuration,
                               available_backends,
                               data_size,
                               memory_budget,
                               algorithm_descriptor)
    IF mode is approximate AND approximation not explicitly permitted:
        RETURN ApproximationNotPermitted
    IF required provenance missing:
        RETURN ExternalArtifactMissingOrInvalid

    // 3. Reusable planning
    plans = BuildOrReusePlans(run_context,
                              spatial_index,
                              geometry,
                              graph,
                              exchangeability,
                              quadrature,
                              mesh,
                              sparse_factorizations)

    // 4. Estimation/inference
    result = algorithm_descriptor.implementation(
                 validated_inputs,
                 design,
                 plans,
                 mode,
                 deterministic_seed_namespace)

    // 5. Diagnostics and calibration metadata
    diagnostics = RunRequiredDiagnostics(result,
                                         algorithm_descriptor,
                                         validated_inputs,
                                         design)
    maturity = DetermineResultMaturity(algorithm_descriptor.maturity,
                                       diagnostics,
                                       provenance_completeness,
                                       mode)

    // 6. Finite and schema boundaries
    ValidateNoNonFinitePersistedValues(result, diagnostics)
    ValidateTypedUnavailableStates(result)
    ValidateClaimLanguage(result, maturity, algorithm_descriptor.claim_limits)

    // 7. Immutable persistence
    artifacts = CommitLargeArtifactsTransactionally(result.large_outputs)
    document = AssembleVersionedResultDocument(result.small_outputs,
                                               diagnostics,
                                               artifacts,
                                               run_context,
                                               maturity)
    CommitResultTransactionally(document, artifacts)
    RETURN document
```

## 123. Backend-selection policy

```text
FUNCTION SelectExecutionMode(config, backends, n, memory_budget, descriptor):
    IF config.mode explicitly requested:
        verify requested mode is supported
        RETURN requested mode

    FOR candidate IN descriptor.modes ordered by scientific preference:
        IF backend available
           AND EstimatedMemory(candidate,n) <= memory_budget
           AND EstimatedRuntime(candidate,n) <= configured_runtime_budget
           AND candidate satisfies requested accuracy:
            RETURN candidate

    IF exact mode unavailable but approximate mode exists:
        RETURN TypedFailure("explicit approximation approval required")
    RETURN ResourceLimitExceeded
```

Examples:

```text
GP:
    exact Cholesky -> sparse inducing/NNGP/SPDE only with explicit mode
Graph filter:
    exact eigendecomposition -> partial Lanczos -> Chebyshev with error bound
Persistent homology:
    exact filtration reduction -> witness/landmark approximation with coverage radius
OT:
    exact linear program -> entropic Sinkhorn with epsilon and marginal residuals
Bayes:
    HMC/NUTS -> SMC/Laplace/VI only with approximation diagnostics
```

## 124. Stable numerical primitives

```text
FUNCTION StableLogSumExp(values):
    m = max(values)
    RETURN m + log(sum(exp(values - m)))

FUNCTION StableLogMeanExp(values):
    RETURN StableLogSumExp(values) - log(len(values))

FUNCTION StableWeightedMean(values, weights):
    normalize weights using compensated/stable summation
    RETURN pairwise_or_compensated_sum(weights*values)

FUNCTION StableCovariance(X, weights OPTIONAL):
    use two-pass or online compensated covariance
    accumulate in f64
    enforce symmetry
    return typed failure if effective sample size insufficient
```

```text
FUNCTION DeterministicParallelReduce(items, map_fn, reduce_fn, seed_namespace):
    partition items deterministically
    process partitions in parallel
    reduce partition outputs in fixed index order
    RETURN bitwise-stable result where backend arithmetic permits,
           otherwise documented tolerance-stable result
```

## 125. Common fitted-model diagnostics

```text
FUNCTION DiagnoseFittedModel(fit, descriptor):
    diagnostics = {}

    IF Bayesian:
        diagnostics += RhatESSMCSE(fit)
        diagnostics += DivergenceAndTreeDepth(fit)
        diagnostics += PriorPredictiveChecks(fit)
        diagnostics += PosteriorPredictiveChecks(fit)
        diagnostics += PriorSensitivity(fit)
        diagnostics += SBCStatus(descriptor)

    IF likelihood/point process:
        diagnostics += residual maps
        diagnostics += count and intensity calibration
        diagnostics += K/L/g/mark posterior predictive checks
        diagnostics += quadrature/integration error

    IF predictive:
        diagnostics += patient-heldout metrics
        diagnostics += calibration
        diagnostics += OOD and abstention
        diagnostics += leakage audit
        diagnostics += subgroup/site/scanner/stain robustness

    IF approximate numerical:
        diagnostics += approximation error bound
        diagnostics += exact-small-case differential
        diagnostics += tolerance sensitivity

    IF causal:
        diagnostics += overlap/positivity
        diagnostics += balance
        diagnostics += negative controls
        diagnostics += sensitivity/partial identification
        diagnostics += assignment-mechanism validation

    RETURN diagnostics
```

## 126. Universal simulation-calibration harness

```text
TYPE CalibrationScenario:
    scenario_id
    generative_model
    parameter_grid_or_prior
    cohort_design
    observation_window
    technical_noise
    registration_noise
    segmentation_noise
    domain_shift
    repetitions
    seed_namespace
    expected_null_or_truth

FUNCTION RunCalibrationSuite(algorithm, scenarios, alpha):
    report = []

    FOR scenario IN scenarios:
        successes = 0
        failures = 0
        estimates = []
        intervals = []
        decisions = []

        FOR r IN 1..scenario.repetitions:
            truth, data = scenario.generate(Seed(CALIBRATION, scenario.id, r))
            fit = algorithm(data)

            IF fit failed:
                failures += 1
                RecordFailureReason()
                CONTINUE

            successes += 1
            estimates.append(fit.estimate)
            intervals.append(fit.interval_or_posterior_interval)
            decisions.append(fit.test_decision)

        metrics = {
            failure_rate,
            bias,
            RMSE,
            interval_coverage,
            type_I_error_if_null,
            power_if_alternative,
            calibration_error,
            posterior_rank_uniformity_if_Bayesian,
            runtime,
            peak_memory
        }
        attach Wilson/binomial uncertainty for rates
        report.append(scenario, metrics, all failures)

    RETURN report
```

## 127. Required generative validation catalogue

```text
CALIBRATION_GENERATORS = {
    homogeneous_Poisson,
    inhomogeneous_Poisson,
    random_labeling,
    Thomas_cluster,
    Matern_cluster,
    Strauss_inhibition,
    Geyer_saturation,
    multitype_attraction,
    multitype_repulsion,
    LGCP,
    compartment_confounding,
    irregular_and_holed_windows,
    boundary_heavy_patterns,
    duplicate_coordinates,
    sparse_and_rare_marks,
    continuous_and_probabilistic_marks,
    correlated_high_dimensional_embeddings,
    graph_signal_controls,
    known_topology,
    multimodal_latent_factor_controls,
    missing_modality_controls,
    registration_and_serial_section_deformation,
    known_clone_geometries_and_trees,
    longitudinal_state_space_controls,
    randomized_interference_controls,
    known_mechanistic_PDE_controls,
    SBI_known_simulators
}
```

## 128. Real-data validation ladder

```text
FUNCTION RealDataValidationLadder(method, datasets):
    Stage 0: synthetic truth and independent numerical oracle
    Stage 1: public benchmark with known technical properties
    Stage 2: internal real cohort with biological/technical negative controls
    Stage 3: heldout patients from same site
    Stage 4: external site/scanner/stain/platform
    Stage 5: prospective or perturbational validation where claim requires it

    method may be promoted only to the highest completed stage
    RETURN validation stage and unresolved generalization risks
```

## 129. Performance and memory benchmark harness

```text
FUNCTION BenchmarkAlgorithmScaling(algorithm, workload_generator, sizes, repetitions):
    results = []
    FOR size n IN sizes:
        workload = workload_generator(n, fixed_density=true,
                                      controlled_output_size=true/recorded)
        warmup and verify checksum
        FOR r IN repetitions:
            measure separately:
                plan construction
                observed evaluation
                one null/posterior step
                full inference
                result assembly
                artifact persistence
                peak resident memory
                allocated bytes where supported
        record output-sensitive quantities:
            pair count, edge count, modes, mesh nodes, factors, particles,
            posterior draws, simulations, filtration simplices
        results.append(medians, intervals, scaling ratios)

    compare with declared complexity and baseline
    RETURN benchmark artifact
```

### 129.1 Required scales

```text
per-commit smoke:       10^3 cells/objects or smaller method-appropriate case
scheduled representative: 10^4–10^5
stable large-tissue gate:  10^6 where endpoint is claimed million-cell capable
streaming/research gate:   10^7–10^8 only for methods specifically designed for it
```

## 130. Failure and availability state mapping

```text
ENUM AnalysisAvailability:
    AVAILABLE
    DISABLED
    NOT_APPLICABLE
    INSUFFICIENT_DATA(reason)
    INVALID_INPUT(reason)
    UNSUPPORTED_DESIGN(reason)
    EXTERNAL_ARTIFACT_MISSING(reason)
    APPROXIMATION_NOT_PERMITTED(reason)
    RESOURCE_LIMIT_EXCEEDED(reason)
    NONCONVERGED(reason, diagnostics_ref)
    NUMERICAL_FAILURE(reason)
    OUT_OF_SIMULATION_SUPPORT(reason)
    PARTIALLY_IDENTIFIED(bounds_ref)
    UNSUPPORTED_FOR_CLAIM(reason)
```

No unavailable state is encoded as `0`, NaN, infinity, an empty vector, or a silently absent field.

## 131. Result maturity determination

```text
FUNCTION DetermineResultMaturity(method_maturity, diagnostics, provenance, mode):
    maturity = method_maturity

    IF provenance incomplete:
        downgrade to unsupported_for_claim
    IF nonconverged or severe diagnostic failure:
        RETURN unsupported_for_claim
    IF approximate mode lacks validated error/calibration:
        downgrade to research_only
    IF external validation absent for predictive clinical claim:
        cap at experimental
    IF causal identification assumptions not supported:
        cap at association_only / unsupported_for_claim
    IF Bayesian SBC or posterior predictive requirements absent:
        cap at experimental

    RETURN maturity with machine-readable reasons
```

## 132. Promotion gates

A method may enter a stable versioned result family only when all applicable conditions are satisfied:

```text
PROMOTION_GATE(method):
    canonical definition and naming frozen
    estimand/target and claim type fixed
    dimensionality, window, units, and edge policy fixed
    null/likelihood/prior/generative model fixed
    randomization and biological replication units fixed
    independent numerical oracle passes
    type-I error and power calibrated for inferential methods
    interval/posterior coverage calibrated where applicable
    SBC and posterior diagnostics pass for Bayesian methods
    exact-small-case differential passes for approximate methods
    patient-heldout/external validation passes for predictive methods
    positivity/negative-control/sensitivity requirements pass for causal methods
    finite persistence and typed states pass
    deterministic seed/thread behavior passes
    memory and runtime budgets pass
    API/config/CLI/result/artifact schemas frozen
    migration and limitations documented
    no unsupported claim language
```

## 133. Kill criteria

```text
KILL_OR_DEFER(method) IF:
    no defined estimand or prediction target
    no credible biological replication unit
    no valid null/likelihood/prior or identification strategy
    no independent oracle or simulation path
    method is dominated by a simpler validated approach
    approximation error cannot be characterized
    model repeatedly fails calibration or posterior predictive checks
    useful inference requires unavailable provenance/data
    computational scaling is incompatible with intended workloads
    output is unstable under minor graph/segmentation/scale perturbations
    predictive gain vanishes under patient-heldout or external validation
    causal claim depends on implausible or unverifiable assumptions
```

---

# Part XIV — Requested-Function Coverage Index

The following table maps every requested function family to its pseudocode owner in this specification.

| Requested capability | Primary section(s) |
|---|---|
| Patient-level permutation | §10 |
| Hierarchical bootstrap | §11 |
| Paired/repeated/multisite designs | §§12–14 |
| Functional two-sample testing | §15 |
| MMD | §16 |
| Energy distance | §17 |
| Equivalence/noninferiority | §§19–22 |
| Hierarchical mixed models/partial pooling | §§23–24 |
| Exact GP and Matérn | §§25–26 |
| Inducing/sparse GP | §27 |
| NNGP | §28 |
| SPDE | §29 |
| CAR/SAR/GMRF/BYM/BYM2 | §30 |
| Spatially varying coefficients | §31 |
| HMC/NUTS/SMC/VI/Laplace/INLA-style | §32 |
| Diagnostics/LOO/SBC | §33 |
| Inhomogeneous Poisson/LGCP | §§34–35 |
| Thomas/Matérn clusters | §36 in Part IV numbering in the earlier document body |
| Strauss/Gibbs/Geyer/exchange MCMC | Part IV interaction-process sections |
| Multitype/joint location–mark | Part IV multitype/mark sections |
| Replicated hierarchical patterns and posterior-predictive K/g | Part IV replicated/PPC sections |
| Vector variograms/covariance | Part V §§27–29 in that part |
| Kernel mark correlation | Part V kernel sections |
| Graph smoothness | Part V graph-signal sections |
| Cell–patch–region complementarity | Part V cell-patch sections |
| Multiscale kernels/fingerprints/retrieval | Part V multiscale and retrieval sections |
| M0–M5 workflows/fusion | Part V predictive/fusion sections |
| Nonrigid/diffeomorphic registration | Part VI registration sections |
| Probabilistic correspondence/uncertainty | Part VI uncertainty sections |
| Partial/unbalanced OT | Part VI OT sections |
| Fused Gromov–Wasserstein | Part VI FGW sections |
| Atlas mapping without identity claims | §35 |
| Graph Fourier/heat kernel | §§37–38 |
| Spectral/diffusion wavelets | §§39–40 |
| Chebyshev approximation | §41 |
| Graph scattering | §42 |
| Heterogeneous graphs/hypergraphs/motifs | §§43–45 |
| Simplicial/cellular complexes | §§46–47 |
| Persistent homology | §49 |
| Alpha/witness complexes | §§50–51 |
| Persistence landscapes/images | §§52–53 |
| Euler/Minkowski functionals | §§54–55 |
| Percolation/connectivity | §56 |
| Perturbation stability | §58 |
| Probabilistic CCA | §61 |
| Multiview factors | §62 |
| Bayesian matrix/tensor factorization | §§63–64 |
| Spatial latent factors | §65 |
| Missing-modality inference | §66 |
| Joint morphology/IHC/omics/clone/clinical | §67 |
| Reaction–diffusion/competition/vascular/front models | §§71–75 |
| Neural Cox/marked point processes | §§76–77 |
| Normalizing flows/diffusion generators | §§78–79 |
| ABC/synthetic likelihood | §§81–82 |
| NPE/NLE/NRE/sequential SBI | §§83–86 |
| SBC/OOD/posterior-predictive lab | §§87–90 |
| Serial-section reconstruction | §§92–93 |
| Anisotropic 3-D windows/processes/fields/graphs/topology | §§94–98 |
| Spatiotemporal state-space models | §99 |
| Deformation versus biological change | §100 |
| Clone phylogeography | §101 |
| Potential outcomes/interference | §§103–105 |
| Spatial propensity and DR/DML | §§106–108 |
| Negative controls/sensitivity/partial ID | §§109–111 |
| Spillover/perturbational workflows | §§112–113 |
| Active ROI/stain/FOV/landmark/replicate allocation | §§114–119 |

The repeated local section numbering in Parts IV and V is retained from the progressive draft structure; implementation should convert these to globally unique requirement IDs rather than relying on document numbers.

---

# Part XV — Complexity and Scaling Summary

| Family | Representative exact cost | Scalable/approximate path | Dominant validation issue |
|---|---:|---|---|
| Restricted permutation | `O(B × estimator)` | shared plans, sequential stopping only if valid | exchangeability and biological unit |
| Hierarchical bootstrap | `O(B × estimator)` | streaming summaries | correct hierarchy |
| MMD | `O(n²)` | block/linear/random-feature MMD | kernel selection and patient unit |
| Energy distance | `O(n²)` | block/energy sketches with error | metric and unit |
| Exact GP | `O(n³)` time, `O(n²)` memory | inducing, NNGP, SPDE | approximation/calibration |
| HMC/NUTS | gradient-dependent | blocked/sparse/VI/Laplace | convergence and geometry |
| LGCP grid/SPDE | sparse factorization dependent | mesh/low-rank/VI | quadrature and latent field |
| Gibbs exact likelihood | intractable normalizer | exchange MCMC/pseudolikelihood | doubly intractable inference |
| Vector variogram | `O(n²d)` | indexed pairs/blocking/projections | high-dimensional stability |
| OT/FGW | `O(nm)` memory; iterative | sparse/landmark/minibatch | entropy/mass/plan nonidentifiability |
| Eigen graph methods | full `O(n³)` | Lanczos/Chebyshev `O(Km)` | graph dependence/error |
| Persistent homology | output/filtration dependent, worst high | sparse filtrations/witness | combinatorial explosion |
| Multimodal factors | `O(observed_entries × K)` per VI epoch | minibatch/sparse | factor identifiability/missingness |
| Mechanistic PDE | mesh × steps × solver | adaptive/multigrid/GPU | solver/model misspecification |
| SBI | simulations dominate | amortization/sequential design | simulation gap/calibration |
| 3-D registration | image/mesh dependent | multiresolution/GPU | deformation uncertainty |
| Interference causal | assignment/exposure dependent | cluster-level influence methods | positivity/identification |
| Active design | candidate × posterior simulations | surrogate/EIG amortization | utility misspecification |

---

# Part XVI — Primary Research Basis and Reference Implementations

The pseudocode above is synthesized from established primary methods, official mathematical formulations, and current reference implementations. The following are the principal sources; implementation tasks should pin exact software versions and licenses separately.

## Cohort-valid inference and comparison

1. Winkler AM, Ridgway GR, Webster MA, Smith SM, Nichols TE. **Permutation inference for the general linear model.** *NeuroImage*. 2014;92:381–397. DOI: https://doi.org/10.1016/j.neuroimage.2014.01.060
2. Myllymäki M, Mrkvička T, Grabarnik P, Seijo H, Hahn U. **Global envelope tests for spatial processes.** *Journal of the Royal Statistical Society: Series B*. 2017;79:381–404. DOI: https://doi.org/10.1111/rssb.12172
3. Gretton A, Borgwardt KM, Rasch MJ, Schölkopf B, Smola A. **A kernel two-sample test.** *JMLR*. 2012;13:723–773. https://jmlr.org/papers/v13/gretton12a.html
4. Székely GJ, Rizzo ML. **Energy statistics: A class of statistics based on distances.** *Journal of Statistical Planning and Inference*. 2013;143:1249–1272. DOI: https://doi.org/10.1016/j.jspi.2013.03.018
5. Schuirmann DJ. **A comparison of the two one-sided tests procedure and the power approach for assessing the equivalence of average bioavailability.** *Journal of Pharmacokinetics and Biopharmaceutics*. 1987;15:657–680. DOI: https://doi.org/10.1007/BF01068419
6. Saravanan V, Berman GJ, Sober SJ. **Application of the hierarchical bootstrap to multi-level data in neuroscience.** *Neuron, Behavior and Data Analysis*. 2020. https://doi.org/10.51628/001c.13927

## Bayesian spatial modeling

7. Lindgren F, Rue H, Lindström J. **An explicit link between Gaussian fields and Gaussian Markov random fields: the stochastic partial differential equation approach.** *JRSS B*. 2011;73:423–498. DOI: https://doi.org/10.1111/j.1467-9868.2011.00777.x
8. Rue H, Martino S, Chopin N. **Approximate Bayesian inference for latent Gaussian models by using integrated nested Laplace approximations.** *JRSS B*. 2009;71:319–392. DOI: https://doi.org/10.1111/j.1467-9868.2008.00700.x
9. Quiñonero-Candela J, Rasmussen CE. **A unifying view of sparse approximate Gaussian process regression.** *JMLR*. 2005;6:1939–1959. https://www.jmlr.org/papers/v6/quinonero-candela05a.html
10. Snelson E, Ghahramani Z. **Sparse Gaussian processes using pseudo-inputs.** *NeurIPS*. 2005.
11. Datta A, Banerjee S, Finley AO, Gelfand AE. **Hierarchical nearest-neighbor Gaussian process models for large geostatistical datasets.** *JASA*. 2016;111:800–812. DOI: https://doi.org/10.1080/01621459.2015.1044091
12. Besag J. **Spatial interaction and the statistical analysis of lattice systems.** *JRSS B*. 1974;36:192–236. DOI: https://doi.org/10.1111/j.2517-6161.1974.tb00999.x
13. Besag J, York J, Mollié A. **Bayesian image restoration, with two applications in spatial statistics.** *Annals of the Institute of Statistical Mathematics*. 1991;43:1–20. DOI: https://doi.org/10.1007/BF00116466
14. Riebler A, Sørbye SH, Simpson D, Rue H. **An intuitive Bayesian spatial model for disease mapping that accounts for scaling.** *Statistical Methods in Medical Research*. 2016;25:1145–1165. DOI: https://doi.org/10.1177/0962280216660421
15. Whittle P. **On stationary processes in the plane.** *Biometrika*. 1954;41:434–449. DOI: https://doi.org/10.1093/biomet/41.3-4.434
16. Hoffman MD, Gelman A. **The No-U-Turn sampler: adaptively setting path lengths in Hamiltonian Monte Carlo.** *JMLR*. 2014;15:1593–1623. https://jmlr.org/papers/v15/hoffman14a.html
17. Del Moral P, Doucet A, Jasra A. **Sequential Monte Carlo samplers.** *JRSS B*. 2006;68:411–436. DOI: https://doi.org/10.1111/j.1467-9868.2006.00553.x
18. Kucukelbir A, Tran D, Ranganath R, Gelman A, Blei DM. **Automatic differentiation variational inference.** *JMLR*. 2017;18:1–45. https://jmlr.org/papers/v18/16-107.html
19. Vehtari A, Gelman A, Gabry J. **Practical Bayesian model evaluation using leave-one-out cross-validation and WAIC.** *Statistics and Computing*. 2017;27:1413–1432. DOI: https://doi.org/10.1007/s11222-016-9696-4
20. Vehtari A, Gelman A, Simpson D, Carpenter B, Bürkner P-C. **Rank-normalization, folding, and localization: an improved R-hat for assessing convergence of MCMC.** *Bayesian Analysis*. 2021;16:667–718. DOI: https://doi.org/10.1214/20-BA1221
21. Talts S, Betancourt M, Simpson D, Vehtari A, Gelman A. **Validating Bayesian inference algorithms with simulation-based calibration.** 2018. arXiv:1804.06788.

## Point processes

22. Møller J, Syversveen AR, Waagepetersen RP. **Log Gaussian Cox processes.** *Scandinavian Journal of Statistics*. 1998;25:451–482. DOI: https://doi.org/10.1111/1467-9469.00115
23. Strauss DJ. **A model for clustering.** *Biometrika*. 1975;62:467–475. DOI: https://doi.org/10.1093/biomet/62.2.467
24. Baddeley A, Turner R. **Practical maximum pseudolikelihood for spatial point patterns.** *Australian & New Zealand Journal of Statistics*. 2000;42:283–322. DOI: https://doi.org/10.1111/1467-842X.00128
25. Murray I, Ghahramani Z, MacKay DJC. **MCMC for doubly-intractable distributions.** *UAI*. 2006.
26. Geyer CJ, Møller J. **Simulation procedures and likelihood inference for spatial point processes.** *Scandinavian Journal of Statistics*. 1994;21:359–373.
27. Berthelsen KK, Møller J. **Likelihood and non-parametric Bayesian MCMC inference for spatial point processes based on perfect simulation and path sampling.** *Scandinavian Journal of Statistics*. 2003;30:549–564. DOI: https://doi.org/10.1111/1467-9469.00348

## Graph and higher-order mathematics

28. Sandryhaila A, Moura JMF. **Discrete signal processing on graphs.** *IEEE Transactions on Signal Processing*. 2013;61:1644–1656. DOI: https://doi.org/10.1109/TSP.2013.2238935
29. Hammond DK, Vandergheynst P, Gribonval R. **Wavelets on graphs via spectral graph theory.** *Applied and Computational Harmonic Analysis*. 2011;30:129–150. DOI: https://doi.org/10.1016/j.acha.2010.04.005
30. Coifman RR, Maggioni M. **Diffusion wavelets.** *Applied and Computational Harmonic Analysis*. 2006;21:53–94. DOI: https://doi.org/10.1016/j.acha.2006.04.004
31. Defferrard M, Bresson X, Vandergheynst P. **Convolutional neural networks on graphs with fast localized spectral filtering.** *NeurIPS*. 2016.
32. Gama F, Ribeiro A, Bruna J. **Diffusion scattering transforms on graphs.** *ICLR*. 2019.
33. Zhou D, Huang J, Schölkopf B. **Learning with hypergraphs: clustering, classification, and embedding.** *NeurIPS*. 2006.
34. Benson AR, Gleich DF, Leskovec J. **Higher-order organization of complex networks.** *Science*. 2016;353:163–166. DOI: https://doi.org/10.1126/science.aad9029
35. Schaub MT, Benson AR, Horn P, Lippner G, Jadbabaie A. **Random walks on simplicial complexes and the normalized Hodge 1-Laplacian.** *SIAM Review*. 2020;62:353–391. DOI: https://doi.org/10.1137/18M1201019
36. Barbarossa S, Sardellitti S. **Topological signal processing over simplicial complexes.** *IEEE Transactions on Signal Processing*. 2020;68:2992–3007. DOI: https://doi.org/10.1109/TSP.2020.2981920

## Registration and optimal transport

37. Beg MF, Miller MI, Trouvé A, Younes L. **Computing large deformation metric mappings via geodesic flows of diffeomorphisms.** *International Journal of Computer Vision*. 2005;61:139–157. DOI: https://doi.org/10.1023/B:VISI.0000043755.93987.aa
38. Avants BB, Epstein CL, Grossman M, Gee JC. **Symmetric diffeomorphic image registration with cross-correlation.** *Medical Image Analysis*. 2008;12:26–41. DOI: https://doi.org/10.1016/j.media.2007.06.004
39. Balakrishnan G, Zhao A, Sabuncu MR, Guttag J, Dalca AV. **VoxelMorph: a learning framework for deformable medical image registration.** *IEEE Transactions on Medical Imaging*. 2019;38:1788–1800. DOI: https://doi.org/10.1109/TMI.2019.2897538
40. Dalca AV, Balakrishnan G, Guttag J, Sabuncu MR. **Unsupervised learning of probabilistic diffeomorphic registration for images and surfaces.** *Medical Image Analysis*. 2019;57:226–236. DOI: https://doi.org/10.1016/j.media.2019.07.006
41. Cuturi M. **Sinkhorn distances: lightspeed computation of optimal transport.** *NeurIPS*. 2013.
42. Chizat L, Peyré G, Schmitzer B, Vialard F-X. **Scaling algorithms for unbalanced optimal transport problems.** *Mathematics of Computation*. 2018;87:2563–2609. DOI: https://doi.org/10.1090/mcom/3303
43. Vayer T, Chapel L, Flamary R, Tavenard R, Courty N. **Optimal transport for structured data with application on graphs.** *ICML*. 2019.
44. Chapel L, Alaya MZ, Gasso G. **Partial optimal transport with applications on positive-unlabeled learning.** *NeurIPS*. 2020.
45. Marsland S, Shardlow T. **Bayesian uncertainty quantification for image registration.** *SIAM/ASA Journal on Uncertainty Quantification*. 2017;5:100–131. DOI: https://doi.org/10.1137/16M1079282

## Topology

46. Edelsbrunner H, Letscher D, Zomorodian A. **Topological persistence and simplification.** *Discrete & Computational Geometry*. 2002;28:511–533. DOI: https://doi.org/10.1007/s00454-002-2885-2
47. Bubenik P. **Statistical topological data analysis using persistence landscapes.** *JMLR*. 2015;16:77–102. https://jmlr.org/papers/v16/bubenik15a.html
48. Adams H, Emerson T, Kirby M, et al. **Persistence images: a stable vector representation of persistent homology.** *JMLR*. 2017;18:1–35. https://jmlr.org/papers/v18/16-337.html
49. Edelsbrunner H, Mücke EP. **Three-dimensional alpha shapes.** *ACM Transactions on Graphics*. 1994;13:43–72. DOI: https://doi.org/10.1145/174462.156635
50. de Silva V, Carlsson G. **Topological estimation using witness complexes.** *Symposium on Point-Based Graphics*. 2004. DOI: https://doi.org/10.2312/SPBG/SPBG04/157-166
51. Cohen-Steiner D, Edelsbrunner H, Harer J. **Stability of persistence diagrams.** *Discrete & Computational Geometry*. 2007;37:103–120. DOI: https://doi.org/10.1007/s00454-006-1276-5

## Multimodal Bayesian modeling

52. Bach FR, Jordan MI. **A probabilistic interpretation of canonical correlation analysis.** Technical Report 688, UC Berkeley. 2005.
53. Argelaguet R, Velten B, Arnol D, et al. **Multi-Omics Factor Analysis—a framework for unsupervised integration of multi-omics data sets.** *Molecular Systems Biology*. 2018;14:e8124. DOI: https://doi.org/10.15252/msb.20178124
54. Argelaguet R, Arnol D, Bredikhin D, et al. **MOFA+: a statistical framework for comprehensive integration of multi-modal single-cell data.** *Genome Biology*. 2020;21:111. DOI: https://doi.org/10.1186/s13059-020-02015-1
55. Hogan JW, Tchernis R. **Bayesian factor analysis for spatially correlated data, with application to summarizing area-level material deprivation from census data.** *Journal of the American Statistical Association*. 2004;99:314–324. DOI: https://doi.org/10.1198/016214504000000296
56. Wang F, Wall MM. **Generalized common spatial factor model.** *Biostatistics*. 2003;4:569–582. DOI: https://doi.org/10.1093/biostatistics/4.4.569
57. Zhang L, Banerjee S. **Spatial factor modeling: A Bayesian matrix-normal approach for misaligned data.** *Biometrics*. 2022;78:560–573. DOI: https://doi.org/10.1111/biom.13452

## Mechanistic and simulation-based inference

58. Turing AM. **The chemical basis of morphogenesis.** *Philosophical Transactions of the Royal Society B*. 1952;237:37–72. DOI: https://doi.org/10.1098/rstb.1952.0012
59. Fisher RA. **The wave of advance of advantageous genes.** *Annals of Eugenics*. 1937;7:355–369. DOI: https://doi.org/10.1111/j.1469-1809.1937.tb02153.x
60. Anderson ARA, Chaplain MAJ. **Continuous and discrete mathematical models of tumor-induced angiogenesis.** *Bulletin of Mathematical Biology*. 1998;60:857–899. DOI: https://doi.org/10.1006/bulm.1998.0042
61. Gatenby RA, Gawlinski ET. **A reaction-diffusion model of cancer invasion.** *Cancer Research*. 1996;56:5745–5753.
62. Wood SN. **Statistical inference for noisy nonlinear ecological dynamic systems.** *Nature*. 2010;466:1102–1104. DOI: https://doi.org/10.1038/nature09319
63. Papamakarios G, Sterratt D, Murray I. **Sequential neural likelihood: fast likelihood-free inference with autoregressive flows.** *AISTATS*. 2019. arXiv:1805.07226.
64. Durkan C, Murray I, Papamakarios G. **On contrastive learning for likelihood-free inference.** *ICML*. 2020.
65. Talts S, Betancourt M, Simpson D, Vehtari A, Gelman A. **Validating Bayesian inference algorithms with simulation-based calibration.** arXiv:1804.06788.
66. Mei H, Eisner J. **The neural Hawkes process: a neurally self-modulating multivariate point process.** *NeurIPS*. 2017.
67. Luo S, Hu W. **Diffusion probabilistic models for 3D point cloud generation.** *CVPR*. 2021. arXiv:2103.01458.

## Causal interference and active design

68. Hudgens MG, Halloran ME. **Toward causal inference with interference.** *JASA*. 2008;103:832–842. DOI: https://doi.org/10.1198/016214508000000292
69. Aronow PM, Samii C. **Estimating average causal effects under general interference, with application to a social network experiment.** *Annals of Applied Statistics*. 2017;11:1912–1947. DOI: https://doi.org/10.1214/16-AOAS1005
70. Chernozhukov V, Chetverikov D, Demirer M, et al. **Double/debiased machine learning for treatment and structural parameters.** *The Econometrics Journal*. 2018;21:C1–C68. DOI: https://doi.org/10.1111/ectj.12097
71. Manski CF. **Identification of treatment response with social interactions.** *The Econometrics Journal*. 2013;16:S1–S23. DOI: https://doi.org/10.1111/j.1368-423X.2012.00368.x
72. Rosenbaum PR. **Sensitivity analysis for certain permutation inferences in matched observational studies.** *Biometrika*. 1987;74:13–26. DOI: https://doi.org/10.1093/biomet/74.1.13
73. Lipsitch M, Tchetgen Tchetgen E, Cohen T. **Negative controls: a tool for detecting confounding and bias in observational studies.** *Epidemiology*. 2010;21:383–388. DOI: https://doi.org/10.1097/EDE.0b013e3181d61eeb
74. Chaloner K, Verdinelli I. **Bayesian experimental design: a review.** *Statistical Science*. 1995;10:273–304. DOI: https://doi.org/10.1214/ss/1177009939
75. Kleinegesse S, Drovandi C, Gutmann MU. **Sequential Bayesian experimental design for implicit models via mutual information.** 2020. arXiv:2003.09379.

## Reference implementations to use as numerical or interoperability oracles

- `spatstat` / R for point processes, windows, simulations, and functional summaries.
- Stan/CmdStan, PyMC, NumPyro, Turing, or equivalent pinned backends for Bayesian differential checks.
- R-INLA/inlabru for SPDE/INLA-style latent Gaussian models.
- GPyTorch/GPflow or equivalent pinned implementations for scalable GP comparisons.
- POT for entropic, unbalanced, partial, and fused optimal-transport comparison.
- GUDHI, Ripser, Dionysus, or a selected pinned TDA implementation for persistence fixtures.
- PyTorch Geometric/DGL or pinned sparse linear-algebra references for graph-learning interoperability.
- `sbi` or a pinned SBI backend for NPE/NLE/NRE differential tests.
- ANTs/SyN, LDDMM, or selected registration backends for deformation and uncertainty fixtures.

These packages are comparators and backends, not semantic authorities by name alone. Marklab must match the chosen estimator, normalization, boundary rule, likelihood, prior, and numerical mode before claiming equivalence.

---

# Part XVII — Implementation Reading Order

The implementation lead should not start with the most fashionable method. The safe dependency order is:

```text
1. identity, hierarchy, windows, artifacts, workflow and provenance
2. cohort design and exchangeability
3. shared Bayesian Model IR and diagnostics
4. cohort-valid functional inference and comparison
5. sparse spatial fields and point-process inference
6. embedding tables and spatial embedding statistics
7. registration uncertainty and transport artifacts
8. graph operators and polynomial spectral methods
9. topology and mathematical morphology
10. multimodal Bayesian factors and missing-modality models
11. mechanistic simulators and SBI laboratory
12. 3-D and longitudinal models
13. causal/interference and active design only with suitable data designs
```

Every method can ultimately run independently or inside a composed workflow, but the shared contracts must be implemented first to prevent incompatible identities, randomization units, numerical modes, diagnostics, and result schemas.

