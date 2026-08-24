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
