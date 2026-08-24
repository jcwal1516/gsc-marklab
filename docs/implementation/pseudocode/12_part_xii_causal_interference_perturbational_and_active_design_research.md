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
