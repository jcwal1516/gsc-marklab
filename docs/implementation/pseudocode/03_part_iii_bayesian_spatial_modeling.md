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

