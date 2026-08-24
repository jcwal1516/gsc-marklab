# Part VII — Graph and Higher-Order Tissue Mathematics

## 36. Canonical graph construction and operator contracts

Graph algorithms are meaningful only after fixing the graph. Every graph result therefore stores a complete `GraphOperatorSpec` and digest.

```text
TYPE GraphOperatorSpec:
    node_domain                  // cells, patches, regions, vessels, mixed entities
    coordinate_frame
    edge_rule                    // radius, kNN, mutual-kNN, Delaunay, learned, anatomical
    physical_radius_um OPTIONAL
    k OPTIONAL
    kernel                       // binary, Gaussian, adaptive, learned
    kernel_bandwidth_um OPTIONAL
    symmetrization               // union, intersection, average, max
    self_loops                   // included or excluded
    normalization                // unnormalized, random-walk, symmetric
    component_policy             // separate, block-diagonal, largest-only
    registration_resolution_policy
    uncertainty_weighting
    graph_digest

TYPE SparseGraphOperator:
    n_nodes
    edge_index[2, m]
    edge_weight[m]
    node_metadata
    degree[n]
    component_id[n]
    spec: GraphOperatorSpec

FUNCTION BuildCanonicalGraph(objects, spec, spatial_index, memory_budget):
    ValidateObjectIdentityAndCoordinates(objects)
    ValidateGraphSpec(spec)

    edges = EMPTY_CANONICAL_EDGE_SET

    SWITCH spec.edge_rule:
        RADIUS:
            FOR each node i:
                spatial_index.visit_within_radius(i, spec.physical_radius_um):
                    j = neighbor.index
                    IF j <= i: CONTINUE
                    weight = EvaluateGraphKernel(i, j, neighbor.distance_um, spec)
                    InsertCanonicalEdge(edges, i, j, weight, memory_budget)

        KNN:
            FOR each node i:
                neighbors = spatial_index.k_nearest(i, spec.k)
                FOR neighbor IN neighbors:
                    InsertDirectedCandidate(i, neighbor.index, weight)
            edges = SymmetrizeCandidates(edges, spec.symmetrization)

        DELAUNAY:
            triangulation = RobustDelaunay(objects.coordinates)
            edges = UniqueUndirectedTriangulationEdges(triangulation)

        ANATOMICAL:
            edges = BuildEdgesUnderCompartmentsAndBarrierRules(objects, spec)

        LEARNED:
            RequireFrozenExternalModelAndTrainingProvenance(spec)
            candidate_edges = GenerateSparseCandidateEdges(spatial_index)
            edges = ScoreAndThresholdCandidateEdges(candidate_edges, spec)

    IF spec.registration_resolution_policy == EXCLUDE_UNRESOLVED:
        edges = FilterEdgesAboveRegistrationResolution(edges, objects)
    ELSE IF spec.registration_resolution_policy == DOWNWEIGHT:
        edges = DownweightByRegistrationUncertainty(edges, objects)

    operator = BuildCompressedSparseOperator(edges, n_nodes)
    operator.component_id = ConnectedComponents(operator)
    operator.graph_digest = Hash(spec, object_ids, edges)
    RETURN operator
```

### 36.1 Laplacian construction

```text
FUNCTION BuildLaplacian(graph, normalization):
    W = SparseSymmetricAdjacency(graph)
    d_i = sum_j W_ij
    D = diag(d)

    SWITCH normalization:
        UNNORMALIZED:
            L = D - W
        RANDOM_WALK:
            REQUIRE d_i > 0 for active nodes
            L = I - D^{-1} W
        SYMMETRIC:
            REQUIRE d_i > 0 for active nodes
            L = I - D^{-1/2} W D^{-1/2}

    HandleIsolatedNodesByDeclaredPolicy(L, graph.spec)
    VerifyPSDWhenExpected(L)
    RETURN SparseLaplacian(L, graph_digest, normalization)
```

## 37. Sparse graph Fourier analysis — **ESTABLISHED / ADVANCED AT SCALE**

For an undirected graph with symmetric Laplacian `L = U Λ Uᵀ`, the graph Fourier transform of signal `x` is `x̂ = Uᵀx`. Exact full eigendecomposition is not acceptable for million-node graphs; use partial spectra or polynomial spectral filters.

### 37.1 Exact or partial graph Fourier transform

```text
FUNCTION GraphFourierTransform(signal x[n, p], laplacian L, spectrum_request):
    ValidateFiniteSignal(x)
    REQUIRE x.rows == L.n

    IF spectrum_request == FULL AND n <= configured_exact_limit:
        (lambda, U) = SymmetricEigendecomposition(L)
    ELSE:
        k = spectrum_request.lowest_k
        (lambda, U) = LanczosSmallestEigenpairs(L, k,
                                                tolerance,
                                                max_iterations,
                                                deterministic_start_seed)
        VerifyEigenResiduals(L, U, lambda)

    x_hat = U^T x
    energy_by_mode = RowSquaredNorm(x_hat)
    RETURN GraphSpectrum(lambda, x_hat, energy_by_mode,
                         exact = (k == n), residuals, graph_digest)
```

### 37.2 Graph-frequency band summaries

```text
FUNCTION SummarizeGraphFrequencyBands(spectrum, band_edges):
    ValidateMonotoneBandEdges(band_edges)
    FOR each band b:
        idx = {k : edge[b] <= lambda_k < edge[b+1]}
        IF idx empty:
            output[b] = InsufficientData("no eigenmodes in band")
        ELSE:
            output[b].energy = sum_k_in_idx energy_by_mode[k]
            output[b].energy_fraction = output[b].energy / total_energy
            output[b].mode_count = |idx|
    RETURN output
```

### 37.3 Graph Fourier null inference

```text
FUNCTION GraphSpectrumNullTest(signal, graph, design, permutations, alpha):
    plan = PrepareExchangeabilityPlan(design)
    observed = ComputeGraphFrequencySummary(signal, graph)

    null_curves = Matrix(permutations, observed.band_count)
    FOR b IN 0..permutations-1:
        permuted_signal = RestrictedPermuteRows(signal, plan, Seed(GRAPH_SPECTRUM, b))
        null_curves[b] = ComputeGraphFrequencySummary(permuted_signal, graph)

    envelope = ExtremeRankLengthEnvelope(observed.curve, null_curves, alpha)
    RETURN observed + envelope + scalar_low_frequency_p_value
```

## 38. Heat-kernel methods — **ESTABLISHED**

The heat operator is `H_t = exp(-tL)`. It summarizes diffusion over graph scale `t`, but `t` is graph-dependent and must be calibrated to physical distance using diffusion profiles or expected squared displacement.

### 38.1 Exact small-graph heat kernel

```text
FUNCTION ExactHeatKernel(L, times):
    (lambda, U) = SymmetricEigendecomposition(L)
    FOR t IN times:
        H_t = U diag(exp(-t * lambda)) U^T
    RETURN {H_t}
```

### 38.2 Matrix-free heat diffusion

```text
FUNCTION ApplyHeatKernel(signal x, L, t, approximation_spec):
    REQUIRE t >= 0
    IF n <= exact_limit:
        RETURN ExactSpectralApply(exp(-t * lambda), x)

    scaled_L, interval = ScaleSpectrumToMinusOneOne(L,
                                                    lambda_max_estimate = LanczosLargestEigenvalue(L))
    coefficients = ChebyshevCoefficients(f(lambda)=exp(-t*lambda),
                                         interval,
                                         approximation_spec.order)
    y = ChebyshevApply(scaled_L, x, coefficients)
    error_bound = BoundChebyshevApproximationError(coefficients, interval)
    RETURN ApproximateSignal(y, error_bound, algorithm="chebyshev_heat")
```

### 38.3 Heat-kernel signatures and diffusion distances

```text
FUNCTION HeatKernelSignature(L, times, node_subset=ALL):
    FOR each t:
        diag_H = EstimateOrComputeHeatKernelDiagonal(L, t)
        signature[:, t] = diag_H[node_subset]
    RETURN signature

FUNCTION DiffusionDistance(i, j, L, t):
    h_i = ApplyHeatKernel(delta_i, L, t)
    h_j = ApplyHeatKernel(delta_j, L, t)
    RETURN WeightedL2(h_i - h_j, stationary_measure)
```

## 39. Spectral graph wavelets — **ESTABLISHED MATHEMATICS; EXPERIMENTAL PATHOLOGY ENDPOINT**

Given a band-pass generating kernel `g` and low-pass `h`, spectral graph wavelets are `ψ_s = g(sL)` and scaling functions are `φ = h(L)`.

```text
TYPE SpectralWaveletSpec:
    scales[]
    bandpass_kernel g
    lowpass_kernel h
    chebyshev_order
    spectral_bound_policy
    normalization

FUNCTION SpectralGraphWaveletTransform(signal x, L, spec):
    lambda_max = EstimateLargestEigenvalue(L)
    ValidateWaveletKernelCoverage(spec, lambda_max)

    scaling = PolynomialSpectralFilter(L, x, h(lambda), spec.chebyshev_order)
    coefficients = []
    errors = []

    FOR s IN spec.scales:
        filter_function(lambda) = g(s * lambda)
        coeff_s, err_s = PolynomialSpectralFilter(L, x, filter_function,
                                                   spec.chebyshev_order)
        coefficients.append(coeff_s)
        errors.append(err_s)

    VerifyFrameBoundsOrRecordUnavailable(spec, lambda_max)
    RETURN GraphWaveletResult(scaling, coefficients, errors,
                              graph_digest, spec)
```

### 39.1 Wavelet energy and localization

```text
FUNCTION GraphWaveletEnergy(coefficients, node_groups OPTIONAL):
    FOR scale s:
        global_energy[s] = sum_i ||coefficients[s][i]||^2
        IF node_groups present:
            FOR group g:
                group_energy[g,s] = sum_i_in_g ||coefficients[s][i]||^2
    RETURN normalized energies and raw energies
```

## 40. Diffusion wavelets — **ADVANCED / EXPERIMENTAL**

Diffusion wavelets construct multiresolution bases by repeatedly compressing powers of a diffusion operator.

```text
FUNCTION BuildDiffusionWaveletTree(graph, tolerance, max_levels):
    T = BuildLazyDiffusionOperator(graph)       // e.g. I - L / lambda_max
    basis_0 = IdentityBasis(n)
    levels = []

    FOR j IN 0..max_levels-1:
        T_power = T^(2^j) represented in current basis
        (Q_j, R_j, retained_rank) = RankRevealingQR(T_power, tolerance)
        scaling_basis = Q_j[:, 0:retained_rank]
        detail_basis = OrthogonalComplementWithinPreviousBasis(scaling_basis)
        compressed_T = scaling_basis^T T_power scaling_basis

        levels.append({scaling_basis, detail_basis, compressed_T,
                       approximation_error})

        IF retained_rank stabilizes OR retained_rank == 1:
            BREAK
        T = compressed_T

    RETURN DiffusionWaveletTree(levels, graph_digest, tolerance)
```

```text
FUNCTION DiffusionWaveletTransform(signal, tree):
    current = signal
    FOR level IN tree.levels:
        detail[level] = level.detail_basis^T current
        current = level.scaling_basis^T current
    RETURN {coarse=current, detail}
```

## 41. Generic Chebyshev polynomial approximation — **ESTABLISHED**

```text
FUNCTION ChebyshevApply(A_scaled, X, coefficients c[0..K]):
    // Spectrum of A_scaled must lie in [-1, 1].
    T0 = X
    Y = 0.5 * c[0] * T0

    IF K == 0:
        RETURN Y

    T1 = A_scaled * X
    Y += c[1] * T1

    FOR k IN 2..K:
        Tk = 2 * A_scaled * T1 - T0
        Y += c[k] * Tk
        T0 = T1
        T1 = Tk

    RETURN Y
```

```text
FUNCTION AdaptiveChebyshevOrder(filter, spectral_interval, tolerance, max_order):
    FOR K IN increasing_orders:
        coefficients = ComputeChebyshevCoefficients(filter, interval, K)
        tail_bound = EstimateCoefficientTail(coefficients)
        IF tail_bound <= tolerance:
            RETURN K, coefficients, tail_bound
    RETURN InsufficientData("requested approximation tolerance not achieved")
```

## 42. Graph scattering — **ADVANCED / EXPERIMENTAL**

Graph scattering cascades wavelet modulus nonlinearities and global or group pooling. It is useful only when graph construction, scale definitions, and invariance are scientifically justified.

```text
FUNCTION GraphScattering(signal X, wavelet_bank, max_order, pooling):
    paths = [{path=[], signal=X}]
    outputs = []

    FOR order IN 0..max_order:
        next_paths = []
        FOR state IN paths:
            pooled = PoolGraphSignal(state.signal, pooling)
            outputs.append({path=state.path, value=pooled})

            IF order == max_order:
                CONTINUE

            FOR scale s IN wavelet_bank.scales:
                IF ViolatesFrequencyOrdering(state.path, s):
                    CONTINUE
                filtered = ApplyWavelet(wavelet_bank[s], state.signal)
                propagated = ElementwiseNormOrModulus(filtered)
                next_paths.append({path=state.path + [s], signal=propagated})
        paths = next_paths

    RETURN ScatteringFeatureVector(outputs, graph_digest, wavelet_spec)
```

```text
FUNCTION ValidateGraphScatteringStability(scattering_model, perturbations):
    FOR perturbation IN perturbations:
        perturbed_graph_or_signal = ApplyPerturbation(perturbation)
        delta = Norm(Scattering(perturbed) - Scattering(original))
        compare delta to perturbation magnitude and predefined tolerance
    RETURN stability_curve
```

## 43. Heterogeneous tissue graphs — **ADVANCED**

```text
TYPE HeterogeneousGraph:
    node_types                // cell, gland, vessel, nerve, compartment, patch, clone region
    node_tables_by_type
    edge_types                // cell-near-cell, cell-in-region, vessel-near-cell, etc.
    sparse_edges_by_relation
    coordinate_frames
    provenance

FUNCTION BuildHeterogeneousTissueGraph(modalities, relation_specs):
    ValidateStableCrossModalityIdentity(modalities)
    graph = EMPTY_HETEROGENEOUS_GRAPH

    FOR modality IN modalities:
        AddTypedNodes(graph, modality)

    FOR relation IN relation_specs:
        SWITCH relation.kind:
            SPATIAL_NEAR:
                edges = RadiusOrKnnRelation(relation.source_type,
                                            relation.target_type,
                                            relation.radius_um)
            CONTAINMENT:
                edges = PointInPolygonOrPatchContainment(relation)
            INTERFACE_CONTACT:
                edges = BoundaryContactRelation(relation)
            SHARED_CLONE:
                edges = ConnectByCloneAssignmentWithUncertainty(relation)
            LEARNED:
                edges = ExternalFrozenRelationModel(relation)
        AddTypedEdges(graph, relation.name, edges)

    ValidateNoIdentityLeakageOrImpossibleRelations(graph)
    RETURN graph
```

### 43.1 Relational message passing contract

```text
FUNCTION HeterogeneousMessagePassing(graph, node_features, relation_parameters):
    FOR layer l:
        messages_by_target = zeros
        FOR relation r = source_type -> target_type:
            FOR edge (i, j) IN graph.edges[r]:
                message = phi_r(node_features[source_type][i],
                                edge_features[r][i,j],
                                relation_parameters[r])
                Aggregate(messages_by_target[target_type][j], message,
                          relation_parameters[r].aggregation)
        FOR node_type t:
            node_features[t] = Update_t(node_features[t], messages_by_target[t])
    RETURN node_features
```

A learned heterogeneous graph model must be patient-held-out, provenance-complete, and separated from descriptive graph statistics.

## 44. Hypergraph methods — **ADVANCED / EXPERIMENTAL**

A hyperedge may encode a gland, niche, clone, patch, compartment, or higher-order cellular interaction.

```text
TYPE Hypergraph:
    incidence H[n_nodes, n_hyperedges] sparse
    hyperedge_weight W_e
    node_degree D_v
    hyperedge_degree D_e
    hyperedge_type
    provenance

FUNCTION BuildHypergraph(nodes, hyperedge_definitions):
    H = SparseIncidenceMatrix()
    FOR each definition e:
        members = ResolveMembersByGeometryLabelsOrImportedAssignment(e)
        REQUIRE |members| >= minimum_members
        FOR node i IN members:
            H[i,e] = MembershipWeight(i,e)
    ComputeDegrees(H)
    RETURN Hypergraph(H, weights, types, provenance)
```

### 44.1 Normalized hypergraph Laplacian

```text
FUNCTION HypergraphLaplacian(hypergraph):
    H = hypergraph.incidence
    W = diag(hypergraph.hyperedge_weight)
    Dv = diag(sum_e W_e * H_ie)
    De = diag(sum_i H_ie)
    Theta = Dv^{-1/2} H W De^{-1} H^T Dv^{-1/2}
    L_h = I - Theta
    RETURN L_h
```

```text
FUNCTION HypergraphSignalSmoothness(signal, L_h):
    RETURN trace(signal^T L_h signal) / max(trace(signal^T signal), epsilon)
```

## 45. Motif-based graph analysis — **ESTABLISHED GRAPH METHOD; ADVANCED PATHOLOGY USE**

```text
FUNCTION CountTypedMotifs(graph, motif_catalog, node_labels):
    counts = zeros(motif_catalog.size)
    FOR motif IN motif_catalog:
        plan = CompileMotifEnumerationPlan(motif, graph.sparsity_structure)
        FOR embedding IN EnumerateSubgraphEmbeddings(graph, plan):
            IF NodeAndEdgeTypesMatch(embedding, motif, node_labels):
                counts[motif] += MotifWeight(embedding)
    RETURN counts
```

```text
FUNCTION BuildMotifAdjacency(graph, motif):
    W_motif = sparse zeros(n,n)
    FOR each motif instance S:
        FOR unordered pair (i,j) IN S.anchor_nodes:
            W_motif[i,j] += 1
    RETURN W_motif
```

```text
FUNCTION MotifNullTest(graph, labels, motif_statistic, null_model, B):
    observed = motif_statistic(graph, labels)
    FOR b IN 0..B-1:
        null_graph_or_labels = SimulateDeclaredGraphNull(graph, labels, null_model, Seed(MOTIF,b))
        null[b] = motif_statistic(null_graph_or_labels)
    RETURN PermutationInference(observed, null)
```

## 46. Simplicial complexes and Hodge operators — **ADVANCED / EXPERIMENTAL**

```text
TYPE SimplicialComplex:
    simplices_by_dimension
    orientation
    boundary_matrix B[k]        // maps k-simplices to (k-1)-simplices
    filtration_value OPTIONAL
    provenance

FUNCTION BuildCliqueComplex(graph, max_dimension):
    simplices[0] = graph.nodes
    simplices[1] = graph.edges
    FOR k IN 2..max_dimension:
        simplices[k] = EnumerateCliquesOfSize(graph, k+1)
        AssignCanonicalOrientation(simplices[k])
    B = BuildBoundaryMatrices(simplices)
    VerifyBoundaryOfBoundaryZero(B)
    RETURN SimplicialComplex(simplices, B)
```

### 46.1 Hodge Laplacians

```text
FUNCTION HodgeLaplacian(complex, k):
    Bk = complex.boundary_matrix[k]
    Bkp1 = complex.boundary_matrix[k+1] IF exists
    L_lower = Bk^T Bk
    L_upper = Bkp1 Bkp1^T IF exists ELSE 0
    L_k = L_lower + L_upper
    RETURN {L_k, L_lower, L_upper}
```

### 46.2 Edge-flow decomposition

```text
FUNCTION HodgeDecomposeEdgeFlow(flow_on_edges, complex):
    L0 = B1 B1^T
    gradient_potential = SolveLeastSquares(L0, B1 * flow)
    gradient_component = B1^T * gradient_potential

    IF triangles exist:
        curl_potential = SolveLeastSquares(B2^T B2, B2^T * flow)
        curl_component = B2 * curl_potential
    ELSE:
        curl_component = 0

    harmonic_component = flow - gradient_component - curl_component
    VerifyOrthogonalityWithinTolerance()
    RETURN {gradient_component, curl_component, harmonic_component}
```

### 46.3 Simplicial signal filtering

```text
FUNCTION FilterKSimplicialSignal(signal, L_k, spectral_filter, approximation):
    RETURN PolynomialSpectralFilter(L_k, signal, spectral_filter, approximation.order)
```

## 47. Cellular complexes — **RESEARCH-ONLY UNTIL PATHOLOGY USE IS FIXED**

```text
FUNCTION BuildCellularComplex(segmented_tissue):
    cells_0 = junctions or selected landmarks
    cells_1 = interfaces or vessel/gland boundary arcs
    cells_2 = compartments, glands, clones, or tissue domains
    boundary_1 = OrientedIncidence(cells_1 -> cells_0)
    boundary_2 = OrientedIncidence(cells_2 -> cells_1)
    VerifyBoundaryOfBoundaryZero(boundary_1, boundary_2)
    RETURN CellularComplex(cells_0, cells_1, cells_2, boundary_matrices)
```

Do not promote a cellular-complex endpoint without a pathology interpretation and robustness under segmentation perturbation.

## 48. Graph-method validation suite

```text
FUNCTION ValidateGraphMathematicsSuite():
    // Independent exact fixtures
    compare Laplacians/eigenpairs against dense reference on small graphs
    compare Chebyshev filters against exact spectral filtering
    verify spectral graph wavelet frame behavior
    verify diffusion-wavelet compression error
    verify scattering stability under controlled graph perturbations
    verify hypergraph Laplacian against hand incidence matrices
    verify motif counts against exhaustive enumeration
    verify B_k * B_{k+1} == 0 for simplicial/cellular complexes
    verify Hodge decomposition reconstruction and orthogonality

    // Scientific stress tests
    vary radius, k, kernel bandwidth, normalization, barriers, and components
    quantify graph-choice sensitivity
    test registration and segmentation perturbations
    test fixed-density scaling and sparse-memory behavior
    test deterministic CPU/GPU parity where GPU is supported

    RETURN validation ledger entries per algorithm and operator spec
```

---

