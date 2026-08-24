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

