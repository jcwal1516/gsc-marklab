#!/usr/bin/env python3
"""SciPy worker for Marklab's advanced Bayesian and SPDE diagnostics.

Each request selects one mode:

* ``hmc_normal``: fixed-step HMC for a normal mean with a conjugate oracle;
* ``spde_suite``: stationary alpha = 2 SPDE LGCP and one-factor MAP fits on rectangular meshes;
* ``adaptive_window_spde`` / ``nonstationary_adaptive_window_spde``: one-factor MAP fits on a
  mesh adapted to a polygonal window, with stationary or piecewise-constant local parameters.
"""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.spatial import Delaunay

from marklab_worker_support.fem import (
    GRADIENT_TOLERANCE,
    LINEAR_ELEMENT_MASS,
    OPTIMIZER_OPTIONS,
    add_element,
    assemble_fem,
    basis_gradients,
    build_mesh,
    factor_objective,
    fit_spatial_factor,
    initial_factor,
    projection,
    sign_aligned_factor,
    sparse_triplets,
    stationary_precision,
    triangle_double_area,
)
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)

REQUEST_FORMAT = "marklab.scipy_advanced_bayes_request"


# Fixed-step HMC


def hmc_normal(spec):
    """Sample a normal-mean posterior with fixed-step Hamiltonian Monte Carlo.

    Model: ``q ~ Normal(prior_mean, prior_sd)`` and ``y_i ~ Normal(q, sigma)`` with known
    ``sigma``. One chain uses momentum ``p ~ Normal(0, sqrt(mass))``, ``leapfrog_steps`` leapfrog
    steps of size ``step_size``, and a Metropolis correction. The conjugate posterior is reported
    alongside the draws as an oracle.
    """
    observations = np.asarray(spec["observations"], dtype=float)
    sigma = float(spec["observation_standard_deviation"])
    prior_mean = float(spec["prior_mean"])
    prior_sd = float(spec["prior_standard_deviation"])
    mass = float(spec["mass"])
    step = float(spec["step_size"])
    leapfrog_steps = int(spec["leapfrog_steps"])
    warmup = int(spec["warmup"])
    draws = int(spec["draws"])
    if (
        not 2 <= len(observations) <= 10000
        or not np.isfinite(observations).all()
        or min(sigma, prior_sd, mass, step) <= 0
        or not 1 <= leapfrog_steps <= 100
        or not 100 <= warmup <= 10000
        or not 500 <= draws <= 100000
    ):
        raise ContractError("bounded finite normal HMC contract violated")
    precision = len(observations) / sigma**2 + 1.0 / prior_sd**2
    analytic_variance = 1.0 / precision
    analytic_mean = analytic_variance * (observations.sum() / sigma**2 + prior_mean / prior_sd**2)

    def log_density(q):
        prior_term = -0.5 * ((q - prior_mean) / prior_sd) ** 2
        return prior_term - 0.5 * float(np.sum(((observations - q) / sigma) ** 2))

    def gradient(q):
        return -(q - prior_mean) / prior_sd**2 + float(np.sum(observations - q)) / sigma**2

    rng = np.random.default_rng(spec["seed"])
    q = float(spec["initial_state"])
    chain = []
    accepted = 0
    energy_errors = []
    for iteration in range(warmup + draws):
        momentum = float(rng.normal(0.0, math.sqrt(mass)))
        proposed_q = q
        proposed_p = momentum + 0.5 * step * gradient(proposed_q)
        for leapfrog in range(leapfrog_steps):
            proposed_q += step * proposed_p / mass
            if leapfrog + 1 < leapfrog_steps:
                proposed_p += step * gradient(proposed_q)
        proposed_p += 0.5 * step * gradient(proposed_q)
        proposed_p = -proposed_p
        current_h = -log_density(q) + momentum**2 / (2.0 * mass)
        proposed_h = -log_density(proposed_q) + proposed_p**2 / (2.0 * mass)
        energy_error = proposed_h - current_h
        if math.log(rng.uniform()) < min(0.0, -energy_error):
            q = proposed_q
            if iteration >= warmup:
                accepted += 1
        if iteration >= warmup:
            chain.append(q)
            energy_errors.append(energy_error)
    values = np.asarray(chain)
    return {
        "format": "marklab.fixed_step_hmc_normal_mean",
        "version": 1,
        "algorithm": {
            "mass": mass,
            "step_size": step,
            "leapfrog_steps": leapfrog_steps,
            "warmup": warmup,
            "draws": draws,
            "seed": spec["seed"],
        },
        "posterior": {
            "mean": float(values.mean()),
            "standard_deviation": float(values.std(ddof=1)),
            "interval_95": np.quantile(values, [0.025, 0.975]).tolist(),
            "draws": values.tolist(),
        },
        "analytic_posterior": {
            "mean": analytic_mean,
            "standard_deviation": math.sqrt(analytic_variance),
        },
        "diagnostics": {
            "acceptance_rate": accepted / draws,
            "maximum_absolute_energy_error": float(np.max(np.abs(energy_errors))),
            "mean_energy_error": float(np.mean(energy_errors)),
            "constraint_transform_jacobian": 0.0,
        },
        "claim_status": "experimental_synthetic_fixed_step_hmc",
    }


# Rectangular SPDE suite


def fit_lgcp(mesh, points, prior_sd, maximum_iterations):
    """Laplace-mode (MAP) fit of a log-Gaussian Cox process with an SPDE latent field.

    Log intensity ``beta + u(s)`` with ``u = sum_k w_k phi_k(s)`` on the mesh basis,
    ``w ~ Normal(0, Q^-1)`` (stationary SPDE GMRF) and ``beta ~ Normal(0, prior_sd)``. The
    Poisson-process likelihood integral uses one-point (centroid) quadrature per triangle; the
    linear predictor is clipped to ``[-30, 30]`` inside the exponential.
    """
    event_projection = projection(mesh, points)
    triangle_coordinates = mesh["vertices"][mesh["triangles"]]
    quadrature_locations = triangle_coordinates.mean(axis=1)
    quadrature_projection = projection(mesh, quadrature_locations)
    weights = mesh["triangle_areas"]
    precision = mesh["precision"]
    count = len(points)

    def objective(parameter):
        beta = parameter[0]
        field = parameter[1:]
        event_eta = beta + event_projection @ field
        quadrature_eta = beta + quadrature_projection @ field
        intensity = np.exp(np.clip(quadrature_eta, -30.0, 30.0))
        value = (
            0.5 * beta**2 / prior_sd**2
            + 0.5 * field @ precision @ field
            - event_eta.sum()
            + weights @ intensity
        )
        beta_gradient = beta / prior_sd**2 - count + weights @ intensity
        field_gradient = (
            precision @ field
            - event_projection.sum(axis=0)
            + quadrature_projection.T @ (weights * intensity)
        )
        return float(value), np.concatenate([[beta_gradient], field_gradient])

    initial = np.zeros(len(mesh["vertices"]) + 1)
    initial[0] = math.log(count / np.sum(weights))
    fit = minimize(
        objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": maximum_iterations, **OPTIMIZER_OPTIONS},
    )
    if not fit.success and np.linalg.norm(fit.jac, ord=np.inf) > GRADIENT_TOLERANCE:
        raise ContractError(f"SPDE LGCP optimization failed: {fit.message}")
    beta = float(fit.x[0])
    field = fit.x[1:]
    vertex_intensity = np.exp(np.clip(beta + field, -30, 30))
    quadrature_intensity = np.exp(np.clip(beta + quadrature_projection @ field, -30, 30))
    x_coordinates = mesh["vertices"][:, 0]
    midpoint = (x_coordinates.min() + x_coordinates.max()) / 2.0
    left = vertex_intensity[x_coordinates < midpoint].mean()
    right = vertex_intensity[x_coordinates > midpoint].mean()
    return {
        "beta": beta,
        "field": field,
        "vertex_intensity": vertex_intensity,
        "integrated": float(weights @ quadrature_intensity),
        "right_left_ratio": float(right / left),
        "objective": float(fit.fun),
        "gradient_max": float(np.linalg.norm(fit.jac, ord=np.inf)),
        "iterations": int(fit.nit),
        "event_projection": event_projection,
        "quadrature_projection": quadrature_projection,
        "quadrature_weights": weights,
    }


def spde_suite(spec):
    window = spec["window"]
    bounds = window["bounds"]
    resolutions = [int(value) for value in spec["mesh_resolutions"]]
    kappa = float(spec["kappa"])
    tau = float(spec["tau"])
    beta_prior_sd = float(spec["lgcp_beta_prior_sd"])
    noise_sd = float(spec["factor_noise_sd"])
    maximum_iterations = int(spec["maximum_iterations"])
    if (
        window["holes"] != []
        or not isinstance(window["frame"], str)
        or not (0 <= bounds[0] < bounds[1] and 0 <= bounds[2] < bounds[3])
        or spec["alpha"] != 2
        or not 3 <= min(resolutions) <= max(resolutions) <= 16
        or resolutions != sorted(set(resolutions))
        or min(kappa, tau, beta_prior_sd, noise_sd) <= 0
        or not 10 <= maximum_iterations <= 10000
    ):
        raise ContractError("bounded hole-free rectangular alpha-two SPDE contract violated")
    points = np.asarray(spec["lgcp_points"], float)
    if (
        points.ndim != 2
        or points.shape[1] != 2
        or not 10 <= len(points) <= 10000
        or not np.isfinite(points).all()
    ):
        raise ContractError("finite bounded LGCP points required")
    sensitivity = []
    fitted_by_resolution = {}
    for resolution in resolutions:
        candidate = build_mesh(bounds, resolution, kappa, tau)
        lgcp = fit_lgcp(candidate, points, beta_prior_sd, maximum_iterations)
        fitted_by_resolution[resolution] = (candidate, lgcp)
        sensitivity.append(
            {
                "resolution": resolution,
                "vertex_count": len(candidate["vertices"]),
                "triangle_count": len(candidate["triangles"]),
                "integrated_intensity": lgcp["integrated"],
                "right_to_left_mean_intensity_ratio": lgcp["right_left_ratio"],
            }
        )
    resolution = resolutions[-1]
    mesh, lgcp = fitted_by_resolution[resolution]
    factor = fit_spatial_factor(mesh, spec["factor_observations"], noise_sd, maximum_iterations)
    eigenvalues = np.linalg.eigvalsh(mesh["precision"])
    max_projection_error = max(
        float(np.max(np.abs(lgcp["event_projection"].sum(axis=1) - 1))),
        float(np.max(np.abs(lgcp["quadrature_projection"].sum(axis=1) - 1))),
        float(np.max(np.abs(factor["projection"].sum(axis=1) - 1))),
    )
    return {
        "format": "marklab.rectangular_spde_suite",
        "version": 1,
        "mesh": {
            "frame": window["frame"],
            "bounds": bounds,
            "holes": [],
            "boundary_condition": "natural_neumann_with_positive_kappa",
            "basis": "piecewise_linear_triangular",
            "alpha": 2,
            "kappa": kappa,
            "tau": tau,
            "resolution": resolution,
            "vertices": mesh["vertices"].tolist(),
            "triangles": mesh["triangles"].tolist(),
            "vertex_count": len(mesh["vertices"]),
            "triangle_count": len(mesh["triangles"]),
            "mass_matrix": sparse_triplets(mesh["mass"]),
            "stiffness_matrix": sparse_triplets(mesh["stiffness"]),
            "precision_matrix": sparse_triplets(mesh["precision"]),
            "minimum_precision_eigenvalue": float(eigenvalues[0]),
            "maximum_projection_row_sum_error": max_projection_error,
        },
        "lgcp": {
            "inference": "fixed_hyperparameter_laplace_map",
            "beta": lgcp["beta"],
            "latent_field_weights": lgcp["field"].tolist(),
            "vertex_intensity": lgcp["vertex_intensity"].tolist(),
            "integrated_intensity": lgcp["integrated"],
            "right_to_left_mean_intensity_ratio": lgcp["right_left_ratio"],
            "objective": lgcp["objective"],
            "gradient_maximum": lgcp["gradient_max"],
            "iterations": lgcp["iterations"],
            "event_projection": sparse_triplets(lgcp["event_projection"]),
            "quadrature_projection": sparse_triplets(lgcp["quadrature_projection"]),
            "quadrature_weights": lgcp["quadrature_weights"].tolist(),
        },
        "spatial_factor": {
            "inference": "fixed_hyperparameter_laplace_map",
            "factor_count": 1,
            "feature_means": factor["means"].tolist(),
            "mesh_field_weights": factor["field"].tolist(),
            "projected_region_factor": factor["projected"].tolist(),
            "loadings": factor["loadings"].tolist(),
            "reconstruction_rmse": factor["rmse"],
            "factor_x_correlation": factor["x_correlation"],
            "objective": factor["objective"],
            "iterations": factor["iterations"],
            "projection": sparse_triplets(factor["projection"]),
        },
        "mesh_sensitivity": sensitivity,
        "limitations": [
            "hole_free_rectangle_only",
            "fixed_spde_hyperparameters",
            "dense_bounded_laplace_map",
            "synthetic_oracle_only",
        ],
        "claim_status": "experimental_synthetic_rectangular_spde_lgcp_and_factor_models",
    }


# Polygonal windows


def signed_ring_area(ring):
    return 0.5 * sum(
        ring[index][0] * ring[index + 1][1] - ring[index + 1][0] * ring[index][1]
        for index in range(len(ring) - 1)
    )


def parse_polygonal_window(value):
    """Validate a GeoJSON MultiPolygon window and return its rings, segments, and exact area."""
    if value.get("type") != "MultiPolygon" or not isinstance(value.get("coordinates"), list):
        raise ContractError("adaptive SPDE window must be a GeoJSON MultiPolygon")
    polygons = []
    boundary_segments = []
    total_area = 0.0
    vertex_count = 0
    for polygon_value in value["coordinates"]:
        if not isinstance(polygon_value, list) or not polygon_value:
            raise ContractError("adaptive SPDE polygon requires an exterior ring")
        rings = []
        for ring_value in polygon_value:
            ring = np.asarray(ring_value, dtype=float)
            if (
                ring.ndim != 2
                or ring.shape[1] != 2
                or len(ring) < 4
                or not np.isfinite(ring).all()
                or not np.array_equal(ring[0], ring[-1])
            ):
                raise ContractError(
                    "adaptive SPDE rings must be finite, closed, and have at least four positions"
                )
            area = abs(float(signed_ring_area(ring)))
            if area <= 0.0:
                raise ContractError("adaptive SPDE ring has zero area")
            rings.append(ring)
            vertex_count += len(ring)
            boundary_segments.extend(
                (ring[index], ring[index + 1]) for index in range(len(ring) - 1)
            )
        exterior_area = abs(float(signed_ring_area(rings[0])))
        hole_area = sum(abs(float(signed_ring_area(ring))) for ring in rings[1:])
        polygon_area = exterior_area - hole_area
        if polygon_area <= 0.0:
            raise ContractError("adaptive SPDE polygon holes consume its exterior")
        total_area += polygon_area
        polygons.append(rings)
    if not polygons or vertex_count > 100000:
        raise ContractError("adaptive SPDE window component/vertex bound violated")
    all_points = np.vstack([ring for polygon in polygons for ring in polygon])
    bounds = [
        float(all_points[:, 0].min()),
        float(all_points[:, 0].max()),
        float(all_points[:, 1].min()),
        float(all_points[:, 1].max()),
    ]
    if not bounds[0] < bounds[1] or not bounds[2] < bounds[3] or not math.isfinite(total_area):
        raise ContractError("adaptive SPDE window bounds or area are invalid")
    return {
        "polygons": polygons,
        "segments": boundary_segments,
        "exact_area": total_area,
        "bounds": bounds,
        "component_count": len(polygons),
        "hole_count": sum(len(polygon) - 1 for polygon in polygons),
        "ring_count": sum(len(polygon) for polygon in polygons),
        "vertex_count": vertex_count,
    }


def point_on_segment(point, first, second, tolerance=1e-10):
    delta = second - first
    relative = point - first
    cross = delta[0] * relative[1] - delta[1] * relative[0]
    scale = max(1.0, float(np.linalg.norm(delta)))
    if abs(float(cross)) > tolerance * scale:
        return False
    dot = float(relative @ delta)
    return -tolerance <= dot <= float(delta @ delta) + tolerance


def point_in_ring(point, ring):
    """Even-odd ray-casting test; returns ``(inside, on_boundary)``."""
    inside = False
    x, y = map(float, point)
    for first, second in zip(ring[:-1], ring[1:], strict=True):
        if point_on_segment(np.asarray(point), first, second):
            return True, True
        if (first[1] > y) != (second[1] > y):
            crossing_x = first[0] + (y - first[1]) * (second[0] - first[0]) / (second[1] - first[1])
            if crossing_x > x:
                inside = not inside
    return inside, False


def window_contains(window, point):
    """Whether ``point`` lies in the closed window (boundaries, including holes', count)."""
    for polygon in window["polygons"]:
        in_exterior, on_exterior = point_in_ring(point, polygon[0])
        if not in_exterior:
            continue
        if on_exterior:
            return True
        excluded = False
        for hole in polygon[1:]:
            in_hole, on_hole = point_in_ring(point, hole)
            if on_hole:
                return True
            if in_hole:
                excluded = True
                break
        if not excluded:
            return True
    return False


def resampled_boundary_points(window, spacing):
    points = []
    for first, second in window["segments"]:
        length = float(np.linalg.norm(second - first))
        divisions = max(1, int(math.ceil(length / spacing)))
        for index in range(divisions):
            points.append(first + (index / divisions) * (second - first))
    return points


def orientation(a, b, c):
    return float((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))


def segment_within_window(window, first, second):
    """Whether a segment crosses no boundary segment and 17 sample points lie in the window."""
    for boundary_first, boundary_second in window["segments"]:
        first_side = orientation(first, second, boundary_first)
        second_side = orientation(first, second, boundary_second)
        boundary_first_side = orientation(boundary_first, boundary_second, first)
        boundary_second_side = orientation(boundary_first, boundary_second, second)
        tolerance = 1e-12 * max(
            1.0,
            float(np.linalg.norm(second - first)),
            float(np.linalg.norm(boundary_second - boundary_first)),
        )
        if first_side * second_side < -(
            tolerance**2
        ) and boundary_first_side * boundary_second_side < -(tolerance**2):
            return False
    return all(
        window_contains(window, first + fraction * (second - first))
        for fraction in np.linspace(0.0, 1.0, 17)
    )


# Window-adapted meshes


def grid_spacing(window, base_resolution):
    x0, x1, y0, y1 = window["bounds"]
    return min((x1 - x0) / (base_resolution - 1), (y1 - y0) / (base_resolution - 1))


def window_candidates(window, base_resolution, boundary_spacing, observation_coordinates):
    """Grid points inside the window, resampled boundary points, and observation locations."""
    x0, x1, y0, y1 = window["bounds"]
    xs = np.linspace(x0, x1, base_resolution)
    ys = np.linspace(y0, y1, base_resolution)
    candidates = [[x, y] for y in ys for x in xs if window_contains(window, np.asarray([x, y]))]
    candidates.extend(resampled_boundary_points(window, boundary_spacing))
    candidates.extend(observation_coordinates.tolist())
    return candidates


def unique_vertices(candidates):
    return np.asarray(sorted({(float(point[0]), float(point[1])) for point in candidates}), float)


def window_triangles(window, vertices, maximum_boundary_checks, maximum_triangles, label):
    """Delaunay-triangulate and keep the triangles whose centroid and edges lie in the window.

    Returns the vertices used by retained triangles, the triangles re-indexed to them, and the
    boundary-segment work count.
    """
    triangulation = Delaunay(vertices)
    boundary_checks = len(triangulation.simplices) * 3 * len(window["segments"])
    if boundary_checks > maximum_boundary_checks:
        raise ContractError(f"{label} boundary-segment/triangle work bound exceeded")
    retained = []
    for simplex in triangulation.simplices:
        triangle = tuple(sorted(map(int, simplex)))
        coordinates = vertices[list(triangle)]
        if not window_contains(window, coordinates.mean(axis=0)):
            continue
        if all(
            segment_within_window(window, coordinates[index], coordinates[(index + 1) % 3])
            for index in range(3)
        ):
            retained.append(triangle)
    triangles = np.asarray(sorted(set(retained)), dtype=int)
    if len(triangles) == 0 or len(triangles) > maximum_triangles:
        raise ContractError(f"{label} triangle bound violated")
    used = sorted(set(map(int, triangles.ravel())))
    remap = {old: new for new, old in enumerate(used)}
    triangles = np.asarray(
        [[remap[int(index)] for index in triangle] for triangle in triangles], dtype=int
    )
    return vertices[used], triangles, boundary_checks


def connected_components(vertex_count, triangles):
    adjacency = [set() for _ in range(vertex_count)]
    for triangle in triangles:
        for left, right in ((0, 1), (1, 2), (2, 0)):
            adjacency[int(triangle[left])].add(int(triangle[right]))
            adjacency[int(triangle[right])].add(int(triangle[left]))
    components = 0
    remaining = set(range(vertex_count))
    while remaining:
        components += 1
        stack = [remaining.pop()]
        while stack:
            current = stack.pop()
            for neighbor in adjacency[current]:
                if neighbor in remaining:
                    remaining.remove(neighbor)
                    stack.append(neighbor)
    return components


def build_adaptive_window_mesh(
    window,
    base_resolution,
    refinement_level,
    observation_coordinates,
    kappa,
    tau,
    maximum_vertices,
    maximum_triangles,
    maximum_boundary_checks,
    memory_budget_bytes,
):
    """Window-adapted mesh with the stationary SPDE precision (``stationary_precision``)."""
    boundary_spacing = grid_spacing(window, base_resolution) / (2**refinement_level)
    boundary_point_count = sum(
        max(1, int(math.ceil(float(np.linalg.norm(second - first)) / boundary_spacing)))
        for first, second in window["segments"]
    )
    if boundary_point_count > 4 * maximum_vertices:
        raise ContractError(
            "adaptive SPDE boundary refinement exceeds the bounded candidate budget"
        )
    candidates = window_candidates(
        window, base_resolution, boundary_spacing, observation_coordinates
    )
    vertices = unique_vertices(candidates)
    if len(vertices) < 3 or len(vertices) > maximum_vertices:
        raise ContractError("adaptive SPDE vertex bound violated")
    vertices, triangles, _ = window_triangles(
        window, vertices, maximum_boundary_checks, maximum_triangles, "adaptive SPDE"
    )
    vertex_count = len(vertices)
    if 10 * vertex_count * vertex_count * 8 > memory_budget_bytes:
        raise ContractError("adaptive SPDE dense finite-element memory budget exceeded")
    mass, stiffness, areas = assemble_fem(vertices, triangles, 1e-14, "adaptive SPDE")
    lumped = mass.sum(axis=1)
    if np.any(lumped <= 0.0):
        raise ContractError("adaptive SPDE mesh contains an unused or zero-mass vertex")
    return {
        "vertices": vertices,
        "triangles": triangles,
        "mass": mass,
        "stiffness": stiffness,
        "precision": stationary_precision(mass, stiffness, lumped, kappa, tau),
        "triangle_areas": np.asarray(areas),
        "mesh_area": float(sum(areas)),
        "connected_components": connected_components(vertex_count, triangles),
        "boundary_spacing": boundary_spacing,
    }


def adaptive_barycentric(coordinates, point):
    """Barycentric coordinates of ``point`` in a triangle, or None for a degenerate triangle."""
    (x0, y0), (x1, y1), (x2, y2) = coordinates
    denominator = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)
    if abs(float(denominator)) <= 1e-14:
        return None
    first = ((y1 - y2) * (point[0] - x2) + (x2 - x1) * (point[1] - y2)) / denominator
    second = ((y2 - y0) * (point[0] - x2) + (x0 - x2) * (point[1] - y2)) / denominator
    return np.asarray([first, second, 1.0 - first - second])


def adaptive_projection(mesh, locations, maximum_visits):
    """Barycentric projection matrix found by a bounded linear search over triangles."""
    locations = np.asarray(locations, dtype=float)
    matrix = np.zeros((len(locations), len(mesh["vertices"])))
    visits = 0
    for row, point in enumerate(locations):
        found = False
        for triangle in mesh["triangles"]:
            visits += 1
            if visits > maximum_visits:
                raise ContractError("adaptive SPDE projection triangle-visit bound exceeded")
            barycentric = adaptive_barycentric(mesh["vertices"][triangle], point)
            if barycentric is None:
                continue
            if barycentric.min() >= -1e-10 and barycentric.max() <= 1.0 + 1e-10:
                matrix[row, triangle] = np.clip(barycentric, 0.0, 1.0)
                matrix[row] /= matrix[row].sum()
                found = True
                break
        if not found:
            raise ContractError("adaptive SPDE observation is not covered by the retained mesh")
    return matrix, visits


def fit_adaptive_spatial_factor(
    mesh, observations, noise_sd, maximum_iterations, maximum_projection_visits
):
    """MAP fit of the one-factor SPDE model (see ``factor_objective``) on an adapted mesh.

    Up to ``min(50, max(1, maximum_iterations // 5))`` exact block-coordinate updates (field given
    loadings, then loadings given field) run first; if the gradient is still above tolerance,
    L-BFGS-B continues from that state with the remaining iteration budget. Its relative-objective
    stop uses ``factr=10`` accuracy; a successful objective-change stop still has to satisfy the
    unchanged gradient acceptance limit.
    """
    coordinates = np.asarray([row["coordinates"] for row in observations], dtype=float)
    values = np.asarray([row["values"] for row in observations], dtype=float)
    design, projection_visits = adaptive_projection(mesh, coordinates, maximum_projection_visits)
    means = values.mean(axis=0)
    centered = values - means
    precision = mesh["precision"]
    field, loadings = initial_factor(design, centered, precision, noise_sd)
    if not np.isfinite(loadings).all() or float(loadings @ loadings) <= 1e-16:
        loadings = np.zeros(values.shape[1])
        loadings[0] = 1.0
    gram = design.T @ design
    converged = False
    objective_value = math.inf
    gradient_maximum = math.inf
    block_iterations = min(50, max(1, maximum_iterations // 5))
    iteration = 0
    while iteration < block_iterations:
        iteration += 1
        loading_norm = float(loadings @ loadings)
        system = precision + (loading_norm / noise_sd**2) * gram + np.eye(len(field)) * 1e-10
        right_hand_side = design.T @ (centered @ loadings) / noise_sd**2
        field = np.linalg.solve(system, right_hand_side)
        projected = design @ field
        denominator = 1.0 + float(projected @ projected) / noise_sd**2
        loadings = (projected @ centered) / (noise_sd**2 * denominator)
        objective_value, gradient = factor_objective(
            field, loadings, design, centered, precision, noise_sd
        )
        gradient_maximum = float(np.linalg.norm(gradient, ord=np.inf))
        if not math.isfinite(objective_value) or not math.isfinite(gradient_maximum):
            raise ContractError("adaptive SPDE block-coordinate solver produced a nonfinite state")
        if gradient_maximum <= GRADIENT_TOLERANCE:
            converged = True
            break
    if not converged:
        vertex_count = len(field)

        def objective(parameter):
            return factor_objective(
                parameter[:vertex_count],
                parameter[vertex_count:],
                design,
                centered,
                precision,
                noise_sd,
            )

        fit = minimize(
            objective,
            np.concatenate([field, loadings]),
            jac=True,
            method="L-BFGS-B",
            options={
                "maxiter": maximum_iterations - block_iterations,
                **OPTIMIZER_OPTIONS,
                "ftol": 10.0 * np.finfo(float).eps,
            },
        )
        field = fit.x[:vertex_count]
        loadings = fit.x[vertex_count:]
        objective_value = float(fit.fun)
        gradient_maximum = float(np.linalg.norm(fit.jac, ord=np.inf))
        iteration = block_iterations + int(fit.nit)
        if (
            not math.isfinite(objective_value)
            or not math.isfinite(gradient_maximum)
            or gradient_maximum > GRADIENT_TOLERANCE
        ):
            raise ContractError(
                f"adaptive SPDE warmed optimization failed after {iteration} iterations "
                f"with gradient {gradient_maximum}: {fit.message}"
            )
    field, loadings, projected = sign_aligned_factor(field, loadings, design @ field)
    reconstruction = means + projected[:, None] * loadings[None, :]
    return {
        "means": means,
        "field": field,
        "projected": projected,
        "loadings": loadings,
        "rmse": float(np.sqrt(np.mean((reconstruction - values) ** 2))),
        "x_correlation": float(np.corrcoef(projected, coordinates[:, 0])[0, 1]),
        "objective": objective_value,
        "gradient_max": gradient_maximum,
        "iterations": iteration,
        "projection": design,
        "projection_visits": projection_visits,
    }


def factor_observation_arrays(spec, window, message):
    """Validate 8-512 unique, finite, in-window observations with 2-16 features each."""
    observations = spec["factor_observations"]
    coordinates = np.asarray([row["coordinates"] for row in observations], dtype=float)
    values = np.asarray([row["values"] for row in observations], dtype=float)
    identities = [row["region_id"] for row in observations]
    if (
        coordinates.ndim != 2
        or coordinates.shape[1] != 2
        or not 8 <= len(coordinates) <= 512
        or values.ndim != 2
        or values.shape[0] != len(coordinates)
        or not 2 <= values.shape[1] <= 16
        or not np.isfinite(coordinates).all()
        or not np.isfinite(values).all()
        or len(set(identities)) != len(identities)
        or any(not isinstance(identity, str) or not identity for identity in identities)
        or any(not window_contains(window, point) for point in coordinates)
    ):
        raise ContractError(message)
    return observations, coordinates, values, identities


def window_summary(window, frame):
    return {
        "frame": frame,
        "exact_area": window["exact_area"],
        "bounds": window["bounds"],
        "component_count": window["component_count"],
        "hole_count": window["hole_count"],
        "ring_count": window["ring_count"],
        "vertex_count": window["vertex_count"],
    }


def require_result_size(result, maximum_result_bytes, label):
    encoded = json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True)
    if len(encoded.encode()) > maximum_result_bytes:
        raise ContractError(f"{label} result exceeds maximum_result_bytes")


def adaptive_window_spde(spec):
    window = parse_polygonal_window(spec["window"])
    frame = spec["window_frame"]
    base_resolution = int(spec["base_resolution"])
    refinement_levels = int(spec["boundary_refinement_levels"])
    maximum_relative_area_error = float(spec["maximum_relative_area_error"])
    kappa = float(spec["kappa"])
    tau = float(spec["tau"])
    noise_sd = float(spec["factor_noise_sd"])
    maximum_iterations = int(spec["maximum_iterations"])
    maximum_vertices = int(spec["maximum_vertices"])
    maximum_triangles = int(spec["maximum_triangles"])
    maximum_boundary_checks = int(spec["maximum_boundary_segment_triangle_checks"])
    maximum_projection_visits = int(spec["maximum_projection_triangle_visits"])
    memory_budget_mib = int(spec["memory_budget_mib"])
    maximum_result_bytes = int(spec["maximum_result_bytes"])
    if (
        not isinstance(frame, str)
        or not frame
        or spec["alpha"] != 2
        or spec["factor_count"] != 1
        or not 4 <= base_resolution <= 32
        or not 0 <= refinement_levels <= 4
        or not all(
            math.isfinite(value) for value in (maximum_relative_area_error, kappa, tau, noise_sd)
        )
        or not 0.0 < maximum_relative_area_error <= 0.25
        or min(kappa, tau, noise_sd) <= 0.0
        or not 10 <= maximum_iterations <= 10000
        or not 16 <= maximum_vertices <= 1024
        or not 16 <= maximum_triangles <= 4096
        or not 1 <= maximum_boundary_checks <= 100000000
        or maximum_projection_visits <= 0
        or not 8 <= memory_budget_mib <= 4096
        or not 1024 <= maximum_result_bytes <= 16 * MIB
    ):
        raise ContractError("adaptive arbitrary-window SPDE controls violate their bounds")
    observations, coordinates, values, identities = factor_observation_arrays(
        spec, window, "adaptive SPDE requires unique finite in-window multivariate observations"
    )
    sensitivity = []
    meshes = []
    for level in range(refinement_levels + 1):
        mesh = build_adaptive_window_mesh(
            window,
            base_resolution,
            level,
            coordinates,
            kappa,
            tau,
            maximum_vertices,
            maximum_triangles,
            maximum_boundary_checks,
            memory_budget_mib * MIB,
        )
        relative_area_error = abs(mesh["mesh_area"] - window["exact_area"]) / window["exact_area"]
        sensitivity.append(
            {
                "boundary_refinement_level": level,
                "boundary_spacing": mesh["boundary_spacing"],
                "vertex_count": len(mesh["vertices"]),
                "triangle_count": len(mesh["triangles"]),
                "mesh_area": mesh["mesh_area"],
                "relative_area_error": relative_area_error,
            }
        )
        meshes.append(mesh)
    mesh = meshes[-1]
    relative_area_error = sensitivity[-1]["relative_area_error"]
    if relative_area_error > maximum_relative_area_error:
        raise ContractError(
            f"adaptive SPDE mesh relative area error {relative_area_error} exceeds tolerance "
            f"{maximum_relative_area_error}"
        )
    factor = fit_adaptive_spatial_factor(
        mesh, observations, noise_sd, maximum_iterations, maximum_projection_visits
    )
    eigenvalues = np.linalg.eigvalsh(mesh["precision"])
    projection_error = float(np.max(np.abs(factor["projection"].sum(axis=1) - 1.0)))
    result = {
        "format": "marklab.adaptive_window_spde",
        "version": 1,
        "window": {
            **window_summary(window, frame),
            "geometry_policy": (
                "exact_polygon_membership_with_reported_piecewise_linear_mesh_area_error"
            ),
        },
        "mesh": {
            "basis": "piecewise_linear_triangular",
            "boundary_condition": "natural_neumann_with_positive_kappa",
            "alpha": 2,
            "kappa": kappa,
            "tau": tau,
            "base_resolution": base_resolution,
            "boundary_refinement_levels": refinement_levels,
            "vertices": mesh["vertices"].tolist(),
            "triangles": mesh["triangles"].tolist(),
            "vertex_count": len(mesh["vertices"]),
            "triangle_count": len(mesh["triangles"]),
            "connected_components": mesh["connected_components"],
            "mesh_area": mesh["mesh_area"],
            "relative_area_error": relative_area_error,
            "constant_mass_integral": float(mesh["mass"].sum()),
            "mass_matrix": sparse_triplets(mesh["mass"]),
            "stiffness_matrix": sparse_triplets(mesh["stiffness"]),
            "precision_matrix": sparse_triplets(mesh["precision"]),
            "minimum_precision_eigenvalue": float(eigenvalues[0]),
            "maximum_precision_eigenvalue": float(eigenvalues[-1]),
            "maximum_projection_row_sum_error": projection_error,
        },
        "spatial_factor": {
            "inference": "fixed_hyperparameter_penalized_map",
            "factor_count": 1,
            "region_ids": identities,
            "feature_count": values.shape[1],
            "feature_means": factor["means"].tolist(),
            "mesh_field_weights": factor["field"].tolist(),
            "projected_region_factor": factor["projected"].tolist(),
            "loadings": factor["loadings"].tolist(),
            "reconstruction_rmse": factor["rmse"],
            "factor_x_correlation": factor["x_correlation"],
            "objective": factor["objective"],
            "gradient_maximum": factor["gradient_max"],
            "iterations": factor["iterations"],
            "projection": sparse_triplets(factor["projection"]),
            "projection_triangle_visits": factor["projection_visits"],
        },
        "mesh_sensitivity": sensitivity,
        "limits": {
            "maximum_vertices": maximum_vertices,
            "maximum_triangles": maximum_triangles,
            "maximum_boundary_segment_triangle_checks": maximum_boundary_checks,
            "maximum_projection_triangle_visits": maximum_projection_visits,
            "memory_budget_mib": memory_budget_mib,
            "maximum_result_bytes": maximum_result_bytes,
            "maximum_iterations": maximum_iterations,
        },
        "assumptions": [
            "polygonal_window_coordinates_are_physical",
            "piecewise_linear_finite_elements",
            "fixed_kappa_tau_and_one_factor",
            "observation_locations_not_population_replicates",
        ],
        "limitations": [
            "boundary_conforming_quality_reported_by_mesh_area_error",
            "fixed_spde_hyperparameters",
            "dense_bounded_penalized_map",
            "within_specimen_spatial_field_diagnostic",
        ],
        "claim_status": "fitted_arbitrary_window_spde_diagnostic",
    }
    require_result_size(result, maximum_result_bytes, "adaptive SPDE")
    return result


# Nonstationary window-adapted SPDE


def anisotropy_tensor(ratio, major_axis_degrees):
    """Unit-determinant tensor ``R diag(ratio, 1/ratio) R^T`` rotated to the major axis."""
    ratio = float(ratio)
    angle = float(major_axis_degrees)
    if (
        not math.isfinite(ratio)
        or not math.isfinite(angle)
        or not 1.0 <= ratio <= 16.0
        or not -180.0 <= angle <= 180.0
    ):
        raise ContractError("nonstationary SPDE anisotropy parameters are invalid")
    radians = math.radians(angle)
    cosine = math.cos(radians)
    sine = math.sin(radians)
    rotation = np.asarray([[cosine, -sine], [sine, cosine]], dtype=float)
    tensor = rotation @ np.diag([ratio, 1.0 / ratio]) @ rotation.T
    tensor = (tensor + tensor.T) / 2.0
    if np.linalg.eigvalsh(tensor)[0] <= 0.0 or abs(float(np.linalg.det(tensor)) - 1.0) > 1e-10:
        raise ContractError("nonstationary SPDE anisotropy tensor is not positive definite")
    return tensor


def nonstationary_parameter_at(point, background, regions):
    matched = [
        region
        for region in regions
        if float(np.linalg.norm(point - region["center"])) <= region["radius"] + 1e-12
    ]
    if len(matched) > 1:
        raise ContractError("nonstationary SPDE parameter regions overlap")
    return matched[0] if matched else background


def local_parameters(raw, region_id, center, radius, refinement_levels):
    return {
        "region_id": region_id,
        "center": center,
        "radius": radius,
        "kappa": float(raw["kappa"]),
        "tau": float(raw["tau"]),
        "anisotropy_ratio": float(raw["anisotropy_ratio"]),
        "major_axis_degrees": float(raw["major_axis_degrees"]),
        "interior_refinement_levels": refinement_levels,
    }


def parameter_regions(spec):
    """Validate the background and the non-overlapping circular parameter regions."""
    background = local_parameters(spec["background"], "background", None, None, 0)
    regions = []
    identities = set()
    for raw in spec["parameter_regions"]:
        identity = raw["region_id"]
        center = np.asarray(raw["center"], dtype=float)
        radius = float(raw["radius"])
        level = int(raw["interior_refinement_levels"])
        region = local_parameters(raw, identity, center, radius, level)
        if (
            not isinstance(identity, str)
            or not identity
            or identity in identities
            or center.shape != (2,)
            or not np.isfinite(center).all()
            or not math.isfinite(radius)
            or radius <= 0.0
            or not 1 <= level <= 4
        ):
            raise ContractError("nonstationary SPDE parameter region is invalid")
        identities.add(identity)
        regions.append(region)
    if not 1 <= len(regions) <= 16:
        raise ContractError("nonstationary SPDE requires one to sixteen parameter regions")
    for left, first in enumerate(regions):
        for second in regions[left + 1 :]:
            separation = float(np.linalg.norm(first["center"] - second["center"]))
            if separation <= first["radius"] + second["radius"]:
                raise ContractError("nonstationary SPDE parameter regions overlap")
    for parameter in [background, *regions]:
        if not all(
            math.isfinite(parameter[key]) and parameter[key] > 0.0 for key in ("kappa", "tau")
        ):
            raise ContractError("nonstationary SPDE kappa and tau must be finite and positive")
        parameter["anisotropy_tensor"] = anisotropy_tensor(
            parameter["anisotropy_ratio"], parameter["major_axis_degrees"]
        )
    return background, regions


def add_region_candidates(candidates, window, regions, base_spacing, maximum_candidates):
    """Append refined grid points inside each circular region (and inside the window)."""
    for region in regions:
        spacing = base_spacing / (2 ** region["interior_refinement_levels"])
        count = int(math.ceil(2.0 * region["radius"] / spacing))
        for row in range(count + 1):
            y = region["center"][1] - region["radius"] + row * spacing
            for column in range(count + 1):
                x = region["center"][0] - region["radius"] + column * spacing
                point = np.asarray([x, y])
                in_region = (
                    float(np.linalg.norm(point - region["center"])) <= region["radius"] + 1e-12
                )
                if in_region and window_contains(window, point):
                    candidates.append([x, y])
                if len(candidates) > maximum_candidates:
                    raise ContractError("nonstationary SPDE adaptive candidate bound exceeded")


def nonstationary_precision(mass, stiffness, lumped, kappas, taus):
    """Nonstationary alpha = 2 SPDE precision ``Q = (K T)^T C~^-1 (K T)``.

    ``K = G_H + diag(C~ kappa^2)`` uses the anisotropic stiffness ``G_H`` and per-vertex
    ``kappa``; ``T = diag(tau)``; ``C~`` is the lumped mass.
    """
    operator = stiffness + np.diag(lumped * kappas**2)
    weighted = operator @ np.diag(taus)
    precision = weighted.T @ np.diag(1.0 / lumped) @ weighted
    return (precision + precision.T) / 2.0


def build_nonstationary_adaptive_mesh(window, spec, observation_coordinates):
    """Window-adapted mesh with piecewise-constant local kappa, tau, and anisotropy.

    Each vertex takes the parameters of the circular region containing it (else the background);
    each triangle's stiffness uses the mean of its vertices' anisotropy tensors.
    """
    base_resolution = int(spec["base_resolution"])
    boundary_level = int(spec["boundary_refinement_levels"])
    maximum_vertices = int(spec["maximum_vertices"])
    maximum_triangles = int(spec["maximum_triangles"])
    maximum_candidates = int(spec["maximum_candidate_points"])
    maximum_boundary_checks = int(spec["maximum_boundary_segment_triangle_checks"])
    memory_budget_bytes = int(spec["memory_budget_mib"]) * MIB
    base_spacing = grid_spacing(window, base_resolution)
    boundary_spacing = base_spacing / (2**boundary_level)
    candidates = window_candidates(
        window, base_resolution, boundary_spacing, observation_coordinates
    )
    background, regions = parameter_regions(spec)
    add_region_candidates(candidates, window, regions, base_spacing, maximum_candidates)
    vertices = unique_vertices(candidates)
    if len(vertices) > maximum_candidates:
        raise ContractError("nonstationary SPDE unique candidate bound exceeded")
    if len(vertices) < 3 or len(vertices) > maximum_vertices:
        raise ContractError("nonstationary SPDE vertex bound violated")
    vertices, triangles, boundary_checks = window_triangles(
        window, vertices, maximum_boundary_checks, maximum_triangles, "nonstationary SPDE"
    )
    vertex_count = len(vertices)
    if 12 * vertex_count * vertex_count * 8 > memory_budget_bytes:
        raise ContractError("nonstationary SPDE dense FEM memory budget exceeded")
    vertex_parameters = [
        nonstationary_parameter_at(point, background, regions) for point in vertices
    ]
    mass = np.zeros((vertex_count, vertex_count))
    stiffness = np.zeros_like(mass)
    areas = []
    triangle_regions = []
    for triangle in triangles:
        coordinates = vertices[triangle]
        double_area = triangle_double_area(coordinates)
        area = abs(double_area) / 2.0
        if area <= 1e-14:
            raise ContractError("nonstationary SPDE produced a degenerate triangle")
        areas.append(area)
        centroid_parameters = nonstationary_parameter_at(
            coordinates.mean(axis=0), background, regions
        )
        triangle_regions.append(centroid_parameters["region_id"])
        gradients = np.column_stack(basis_gradients(coordinates, double_area))
        tensor_sum = sum(
            (vertex_parameters[int(index)]["anisotropy_tensor"] for index in triangle),
            np.zeros((2, 2)),
        )
        tensor = tensor_sum / 3.0
        add_element(mass, triangle, area / 12.0 * LINEAR_ELEMENT_MASS)
        add_element(stiffness, triangle, area * gradients @ tensor @ gradients.T)
    areas = np.asarray(areas)
    lumped = mass.sum(axis=1)
    if np.any(lumped <= 0.0):
        raise ContractError("nonstationary SPDE mesh has zero mass")
    kappas = np.asarray([parameter["kappa"] for parameter in vertex_parameters])
    taus = np.asarray([parameter["tau"] for parameter in vertex_parameters])
    precision = nonstationary_precision(mass, stiffness, lumped, kappas, taus)
    symmetry_error = float(np.max(np.abs(precision - precision.T)))
    eigenvalues = np.linalg.eigvalsh(precision)
    if eigenvalues[0] <= 0.0:
        raise ContractError("nonstationary SPDE precision is not positive definite")
    summaries = []
    for parameter in [background, *regions]:
        selected = areas[np.asarray(triangle_regions) == parameter["region_id"]]
        if len(selected) == 0:
            raise ContractError(
                f"nonstationary SPDE region {parameter['region_id']} has no retained triangles"
            )
        summaries.append(
            {
                "region_id": parameter["region_id"],
                "kappa": parameter["kappa"],
                "tau": parameter["tau"],
                "anisotropy_ratio": parameter["anisotropy_ratio"],
                "major_axis_degrees": parameter["major_axis_degrees"],
                "anisotropy_tensor": parameter["anisotropy_tensor"].tolist(),
                "interior_refinement_levels": parameter["interior_refinement_levels"],
                "triangle_count": len(selected),
                "median_triangle_area": float(np.median(selected)),
            }
        )
    return {
        "vertices": vertices,
        "triangles": triangles,
        "mass": mass,
        "stiffness": stiffness,
        "precision": precision,
        "mesh_area": float(areas.sum()),
        "boundary_spacing": boundary_spacing,
        "minimum_precision_eigenvalue": float(eigenvalues[0]),
        "maximum_precision_eigenvalue": float(eigenvalues[-1]),
        "maximum_precision_symmetry_error": symmetry_error,
        "summaries": summaries,
        "background": summaries[0],
        "regions": summaries[1:],
        "boundary_checks": boundary_checks,
    }


def nonstationary_adaptive_window_spde(spec):
    window = parse_polygonal_window(spec["window"])
    frame = spec["window_frame"]
    maximum_relative_area_error = float(spec["maximum_relative_area_error"])
    noise_sd = float(spec["factor_noise_sd"])
    maximum_iterations = int(spec["maximum_iterations"])
    maximum_projection_visits = int(spec["maximum_projection_triangle_visits"])
    maximum_result_bytes = int(spec["maximum_result_bytes"])
    if (
        not isinstance(frame, str)
        or not frame
        or spec["alpha"] != 2
        or spec["factor_count"] != 1
        or not 4 <= int(spec["base_resolution"]) <= 32
        or not 0 <= int(spec["boundary_refinement_levels"]) <= 4
        or not math.isfinite(maximum_relative_area_error)
        or not 0.0 < maximum_relative_area_error <= 0.25
        or not math.isfinite(noise_sd)
        or noise_sd <= 0.0
        or not 10 <= maximum_iterations <= 10000
        or not 16 <= int(spec["maximum_vertices"]) <= 1024
        or not 16 <= int(spec["maximum_triangles"]) <= 4096
        or not 16 <= int(spec["maximum_candidate_points"]) <= 8192
        or not 1 <= int(spec["maximum_boundary_segment_triangle_checks"]) <= 100000000
        or not 1 <= maximum_projection_visits <= 100000000
        or not 8 <= int(spec["memory_budget_mib"]) <= 4096
        or not 1024 <= maximum_result_bytes <= 16 * MIB
    ):
        raise ContractError("nonstationary adaptive SPDE controls violate their bounds")
    observations, coordinates, values, identities = factor_observation_arrays(
        spec, window, "nonstationary SPDE observations are invalid"
    )
    mesh = build_nonstationary_adaptive_mesh(window, spec, coordinates)
    relative_area_error = abs(mesh["mesh_area"] - window["exact_area"]) / window["exact_area"]
    if relative_area_error > maximum_relative_area_error:
        raise ContractError(
            f"nonstationary SPDE mesh relative area error {relative_area_error} exceeds "
            f"{maximum_relative_area_error}"
        )
    factor = fit_adaptive_spatial_factor(
        mesh, observations, noise_sd, maximum_iterations, maximum_projection_visits
    )
    projection_error = float(np.max(np.abs(factor["projection"].sum(axis=1) - 1.0)))
    result = {
        "format": "marklab.nonstationary_adaptive_window_spde",
        "version": 1,
        "statistical_unit": "within_specimen_field_diagnostic",
        "window": window_summary(window, frame),
        "mesh": {
            "operator": "mass_lumped_nonstationary_alpha_two_spde",
            "basis": "piecewise_linear_triangular",
            "boundary_condition": "natural_neumann_with_positive_local_kappa",
            "alpha": 2,
            "vertices": mesh["vertices"].tolist(),
            "triangles": mesh["triangles"].tolist(),
            "vertex_count": len(mesh["vertices"]),
            "triangle_count": len(mesh["triangles"]),
            "mesh_area": mesh["mesh_area"],
            "relative_area_error": relative_area_error,
            "mass_matrix": sparse_triplets(mesh["mass"]),
            "anisotropic_stiffness_matrix": sparse_triplets(mesh["stiffness"]),
            "precision_matrix": sparse_triplets(mesh["precision"]),
            "minimum_precision_eigenvalue": mesh["minimum_precision_eigenvalue"],
            "maximum_precision_eigenvalue": mesh["maximum_precision_eigenvalue"],
            "maximum_precision_symmetry_error": mesh["maximum_precision_symmetry_error"],
            "maximum_projection_row_sum_error": projection_error,
        },
        "background": mesh["background"],
        "parameter_regions": mesh["regions"],
        "spatial_factor": {
            "inference": "fixed_local_hyperparameter_penalized_map",
            "factor_count": 1,
            "region_ids": identities,
            "feature_count": values.shape[1],
            "feature_means": factor["means"].tolist(),
            "mesh_field_weights": factor["field"].tolist(),
            "projected_region_factor": factor["projected"].tolist(),
            "loadings": factor["loadings"].tolist(),
            "reconstruction_rmse": factor["rmse"],
            "objective": factor["objective"],
            "gradient_maximum": factor["gradient_max"],
            "iterations": factor["iterations"],
            "projection": sparse_triplets(factor["projection"]),
            "projection_triangle_visits": factor["projection_visits"],
        },
        "limits": {
            "maximum_vertices": int(spec["maximum_vertices"]),
            "maximum_triangles": int(spec["maximum_triangles"]),
            "maximum_candidate_points": int(spec["maximum_candidate_points"]),
            "maximum_boundary_segment_triangle_checks": int(
                spec["maximum_boundary_segment_triangle_checks"]
            ),
            "maximum_projection_triangle_visits": maximum_projection_visits,
            "memory_budget_mib": int(spec["memory_budget_mib"]),
            "maximum_result_bytes": maximum_result_bytes,
            "maximum_iterations": maximum_iterations,
        },
        "assumptions": [
            "piecewise_constant_declared_local_spde_parameters",
            "nonoverlapping_circular_parameter_regions",
            "piecewise_linear_finite_elements_with_mass_lumping",
            "observation_locations_are_not_population_replicates",
        ],
        "limitations": [
            "local_parameters_are_prespecified_not_estimated",
            "within_specimen_spatial_field_diagnostic",
            "dense_bounded_penalized_map",
        ],
        "claim_status": "fitted_nonstationary_anisotropic_adaptive_spde_diagnostic",
    }
    require_result_size(result, maximum_result_bytes, "nonstationary SPDE")
    return result


# Entry point

MODES = {
    "hmc_normal": hmc_normal,
    "spde_suite": spde_suite,
    "adaptive_window_spde": adaptive_window_spde,
    "nonstationary_adaptive_window_spde": nonstationary_adaptive_window_spde,
}
WINDOW_MODES = {"adaptive_window_spde", "nonstationary_adaptive_window_spde"}


def main():
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != REQUEST_FORMAT or request["version"] != 1:
        raise ContractError("request identity/size mismatch")
    mode = request["mode"]
    if mode not in MODES:
        raise ContractError("unknown advanced Bayesian mode")
    result = MODES[mode](request["spec"])
    if mode in WINDOW_MODES:
        canonical_area = float(request["canonical_window_area"])
        if abs(result["window"]["exact_area"] - canonical_area) > 1e-10 * max(1.0, canonical_area):
            raise ContractError("Rust and SciPy adaptive SPDE window areas disagree")
        result["window"]["exact_area"] = canonical_area
        result["window"]["canonical_sha256"] = request["canonical_window_sha256"]
    result["backend"] = request["backend"]
    result["request_sha256"] = sha256_hex(request_bytes)
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "advanced Bayesian worker contract error", errors=CONTRACT_ERRORS)
