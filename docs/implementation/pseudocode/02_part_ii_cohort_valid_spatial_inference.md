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

