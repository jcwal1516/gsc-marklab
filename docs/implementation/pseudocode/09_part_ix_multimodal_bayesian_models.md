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
