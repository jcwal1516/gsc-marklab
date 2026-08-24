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

