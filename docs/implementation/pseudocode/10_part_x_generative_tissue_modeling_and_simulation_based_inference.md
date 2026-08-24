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
