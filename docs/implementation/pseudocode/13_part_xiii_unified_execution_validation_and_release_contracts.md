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

