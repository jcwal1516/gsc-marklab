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
