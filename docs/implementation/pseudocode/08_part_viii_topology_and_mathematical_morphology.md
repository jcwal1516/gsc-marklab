# Part VIII — Topology and Mathematical Morphology

## 49. Filtrations and persistent homology — **ESTABLISHED MATHEMATICS; ADVANCED PATHOLOGY USE**

### 49.1 Generic filtration contract

```text
TYPE Filtration:
    simplices ordered by nondecreasing filtration_value
    tie_break_order
    boundary_columns
    coefficient_field
    dimension_limit
    provenance

FUNCTION ValidateFiltration(filtration):
    REQUIRE every face appears no later than its coface
    REQUIRE finite filtration values
    REQUIRE deterministic tie order
    REQUIRE boundary-of-boundary zero
```

### 49.2 Standard matrix-reduction persistence algorithm

```text
FUNCTION PersistentHomology(filtration):
    ValidateFiltration(filtration)
    R = CopyBoundaryMatrixColumns(filtration)
    V = IdentityChangeOfBasis OPTIONAL
    low_to_column = EMPTY_MAP
    pairs = []

    FOR column j IN filtration order:
        WHILE R[j] is nonempty:
            i = LowestNonzeroRow(R[j])
            IF i NOT IN low_to_column:
                BREAK
            k = low_to_column[i]
            factor = Coefficient(R[j], i) / Coefficient(R[k], i)
            R[j] = R[j] - factor * R[k]        // over selected field
            IF representative_cycles_requested:
                V[j] = V[j] - factor * V[k]

        IF R[j] is nonempty:
            i = LowestNonzeroRow(R[j])
            low_to_column[i] = j
            birth = filtration.value[i]
            death = filtration.value[j]
            dimension = filtration.simplex_dimension[i]
            pairs.append((dimension, birth, death, i, j))
        ELSE:
            MarkAsPotentialEssentialBirth(j)

    FOR unpaired zero columns j:
        pairs.append((dimension(j), filtration.value[j], +infinity, j, NONE))

    RETURN PersistenceDiagramByDimension(pairs, optional_representatives)
```

Production code should use a proven persistence library or rigorously validated reduction backend; the pseudocode defines the contract and validation oracle.

## 50. Alpha complexes — **ESTABLISHED**

```text
FUNCTION BuildAlphaFiltration(points, max_alpha, dimension):
    REQUIRE Euclidean coordinates and valid dimensionality
    delaunay = DelaunayTriangulation(points)
    simplices = []

    FOR simplex sigma IN delaunay.simplices up to dimension:
        alpha_value = SquaredRadiusOfSmallestEmptyCircumsphere(sigma, delaunay)
        IF alpha_value <= max_alpha^2:
            AddSimplexAndFacesWithFiltrationValue(simplices, sigma, alpha_value)

    ResolveDegeneraciesByExactOrRobustPredicates()
    SortByFiltrationThenFaceOrder(simplices)
    RETURN Filtration(simplices, physical_scale = sqrt(alpha_value))
```

Weighted alpha complexes may incorporate cell radii or uncertainty only under a separate validated contract.

## 51. Witness complexes — **ADVANCED / EXPERIMENTAL**

Useful for subsampling very large point clouds, but approximation error and landmark selection must be recorded.

```text
FUNCTION BuildWitnessFiltration(points, landmark_spec, max_dimension, nu, max_scale):
    landmarks = SelectLandmarks(points, landmark_spec)   // farthest-point or stratified
    witnesses = points
    distance_table = NearestLandmarkDistances(witnesses, landmarks, max_needed)

    simplices = all vertices at filtration 0
    FOR candidate landmark simplex sigma up to max_dimension:
        value = MinimumWitnessRadiusSatisfyingNuWitnessCondition(sigma,
                                                                  witnesses,
                                                                  distance_table,
                                                                  nu)
        IF value <= max_scale:
            AddSimplexAndFaces(simplices, sigma, value)

    RETURN Filtration(simplices,
                      approximation={landmark_count, coverage_radius, nu})
```

## 52. Persistence landscapes — **ESTABLISHED**

For each finite persistence pair `(b,d)`, define a tent function `f(t)=max(0,min(t-b,d-t))`; the `k`th landscape is the `k`th largest tent value.

```text
FUNCTION PersistenceLandscape(diagram, grid, max_k):
    finite_pairs = RemoveEssentialAndZeroPersistencePairs(diagram)
    landscapes[max_k, |grid|] = 0

    FOR grid index q, t IN grid:
        values = []
        FOR (birth, death) IN finite_pairs:
            values.append(max(0, min(t - birth, death - t)))
        sort values descending
        FOR k IN 0..min(max_k, values.length)-1:
            landscapes[k,q] = values[k]

    RETURN landscapes with grid and norm convention
```

## 53. Persistence images — **ESTABLISHED**

```text
FUNCTION PersistenceImage(diagram, birth_grid, persistence_grid, kernel_bandwidth, weight_fn):
    image = zeros(len(birth_grid), len(persistence_grid))

    FOR finite pair (b,d) IN diagram:
        p = d - b
        IF p <= 0: CONTINUE
        weight = weight_fn(b,p)
        FOR pixels within kernel truncation radius:
            image[pixel] += weight * IntegratedGaussianOverPixel(center=(b,p),
                                                                 bandwidth=kernel_bandwidth)

    NormalizeByDeclaredPolicy(image)
    RETURN image with transform and pixel-area metadata
```

The weight function and bandwidth must be fitted only within training folds for predictive use.

## 54. Euler characteristic curves — **ESTABLISHED**

For a finite cell/simplicial complex, `χ = Σ_k (-1)^k N_k`.

```text
FUNCTION EulerCharacteristicCurve(filtration, thresholds):
    counts_by_dimension = zeros(max_dim+1)
    events = filtration.simplices sorted by value
    cursor = 0

    FOR threshold t IN thresholds ascending:
        WHILE cursor < events.length AND events[cursor].value <= t:
            k = events[cursor].dimension
            counts_by_dimension[k] += 1
            cursor += 1
        chi[t] = sum_k (-1)^k * counts_by_dimension[k]

    RETURN chi curve and simplex counts
```

## 55. Minkowski functionals and mathematical morphology — **ESTABLISHED FOR BINARY SETS**

In 2-D, the primary additive functionals are area, perimeter, and Euler characteristic; report conventions explicitly.

```text
FUNCTION MinkowskiFunctionals2D(binary_set, representation, scale):
    IF representation == POLYGON:
        area = ExactPolygonArea(binary_set)
        perimeter = ExactBoundaryLength(binary_set)
        euler = Components(binary_set) - Holes(binary_set)
    ELSE IF representation == RASTER:
        area = PixelAreaEstimator(binary_set, scale)
        perimeter = CalibratedCroftonPerimeter(binary_set, scale)
        euler = DigitalEulerCharacteristic(binary_set, connectivity_convention)

    RETURN {area, perimeter, euler,
            normalized_perimeter=perimeter/sqrt(area),
            conventions, uncertainty}
```

### 55.1 Dilation/erosion curves

```text
FUNCTION MorphologicalFunctionalCurve(set, radii_um):
    FOR r IN radii_um:
        dilated = MinkowskiDilation(set, disk_radius=r)
        eroded  = MinkowskiErosion(set, disk_radius=r)
        output.dilation[r] = MinkowskiFunctionals2D(dilated)
        output.erosion[r]  = MinkowskiFunctionals2D(eroded)
    RETURN output
```

## 56. Percolation and connectivity transitions — **ADVANCED / EXPERIMENTAL PATHOLOGY USE**

```text
FUNCTION ConnectivityTransition(points, radii_um, window):
    spatial_index = BuildSpatialIndex(points)
    union_find = InitializeDisjointSet(n)
    edge_events = GeneratePairDistanceEventsUpToMaxRadius(spatial_index, max(radii_um))
    sort edge_events by distance
    cursor = 0

    FOR r IN radii_um ascending:
        WHILE cursor < edge_events.length AND edge_events[cursor].distance <= r:
            union_find.union(edge.source, edge.target)
            cursor += 1

        component_sizes = union_find.component_sizes()
        largest_fraction[r] = max(component_sizes) / n
        n_components[r] = len(component_sizes)
        susceptibility[r] = sum_{components excluding largest} size^2 / n
        spans_window[r] = DetectBoundarySpanningComponent(union_find, points, window)

    critical_radius = EstimateTransitionRadius(largest_fraction,
                                               susceptibility,
                                               spans_window)
    RETURN curves + critical_radius + finite-size caveats
```

For cohort inference, compare patient-level transition summaries; do not treat each edge event as a replicate.

## 57. Topological two-sample comparison

```text
FUNCTION ComparePersistenceDistributions(group_A_diagrams, group_B_diagrams, design, metric):
    REQUIRE independent biological-unit diagrams or fingerprints

    distance_matrix = PairwiseDiagramDistance(all_diagrams,
                                              metric = bottleneck | Wasserstein | landscape_L2)
    statistic = EnergyStatisticFromDistanceMatrix(distance_matrix, group_labels)
    p_value = RestrictedPatientPermutation(statistic, design)

    RETURN statistic, p_value, distance_matrix_artifact, metric_spec
```

## 58. Stability under segmentation and scale perturbation

```text
FUNCTION TopologyStabilityLaboratory(base_segmentation,
                                     perturbation_generator,
                                     scales,
                                     repetitions):
    baseline_outputs = ComputeTopologyAndMorphology(base_segmentation, scales)
    results = []

    FOR r IN 0..repetitions-1:
        perturbed = perturbation_generator(base_segmentation, Seed(TOPOLOGY_PERTURB,r))
        outputs = ComputeTopologyAndMorphology(perturbed, scales)

        results.append({
            diagram_bottleneck = BottleneckDistance(outputs.diagram,
                                                     baseline_outputs.diagram),
            landscape_L2 = L2(outputs.landscape - baseline_outputs.landscape),
            euler_curve_Linf = MaxAbs(outputs.euler - baseline_outputs.euler),
            minkowski_relative_error = RelativeError(outputs.functionals,
                                                       baseline_outputs.functionals),
            critical_radius_shift = outputs.percolation_radius
                                    - baseline_outputs.percolation_radius
        })

    RETURN distributions, quantiles, failure rates, sensitivity flags
```

## 59. Topology/morphology validation suite

```text
FUNCTION ValidateTopologySuite():
    use hand complexes with known Betti numbers
    compare alpha-complex persistence to a trusted reference implementation
    verify landscape and persistence-image transforms against independent code
    verify Euler characteristic by both Betti numbers and alternating simplex counts
    compare polygon and high-resolution raster Minkowski functionals
    verify percolation curves on lattice and random geometric graph controls
    test stability under bounded coordinate and segmentation perturbations
    test memory scaling and sparse reduction behavior
    record essential-class and infinite-death policy explicitly
```

---
