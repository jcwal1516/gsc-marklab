#!/usr/bin/env python3

import hashlib
import itertools
import json
import math
import sys

import numpy as np
import scipy
from scipy.cluster.vq import kmeans2
from scipy.optimize import minimize
from scipy.spatial import Delaunay


class ContractError(Exception):
    pass


def hmc_normal(spec):
    observations = np.asarray(spec["observations"], dtype=float)
    sigma = float(spec["observation_standard_deviation"])
    prior_mean = float(spec["prior_mean"])
    prior_sd = float(spec["prior_standard_deviation"])
    mass = float(spec["mass"])
    step = float(spec["step_size"])
    leapfrog_steps = int(spec["leapfrog_steps"])
    warmup = int(spec["warmup"]); draws = int(spec["draws"])
    if not 2 <= len(observations) <= 10000 or not np.isfinite(observations).all() or min(sigma, prior_sd, mass, step) <= 0 or not 1 <= leapfrog_steps <= 100 or not 100 <= warmup <= 10000 or not 500 <= draws <= 100000:
        raise ContractError("bounded finite normal HMC contract violated")
    precision = len(observations) / sigma**2 + 1.0 / prior_sd**2
    analytic_variance = 1.0 / precision
    analytic_mean = analytic_variance * (observations.sum() / sigma**2 + prior_mean / prior_sd**2)
    def logp(q):
        return -0.5 * ((q - prior_mean) / prior_sd) ** 2 - 0.5 * float(np.sum(((observations - q) / sigma) ** 2))
    def gradient(q):
        return -(q - prior_mean) / prior_sd**2 + float(np.sum(observations - q)) / sigma**2
    rng = np.random.default_rng(spec["seed"]); q = float(spec["initial_state"])
    chain = []; accepted = 0; energy_errors = []
    for iteration in range(warmup + draws):
        momentum = float(rng.normal(0.0, math.sqrt(mass)))
        current_q = q; current_p = momentum
        proposed_q = q; proposed_p = momentum + 0.5 * step * gradient(proposed_q)
        for leapfrog in range(leapfrog_steps):
            proposed_q += step * proposed_p / mass
            if leapfrog + 1 < leapfrog_steps:
                proposed_p += step * gradient(proposed_q)
        proposed_p += 0.5 * step * gradient(proposed_q); proposed_p = -proposed_p
        current_h = -logp(current_q) + current_p**2 / (2.0 * mass)
        proposed_h = -logp(proposed_q) + proposed_p**2 / (2.0 * mass)
        energy_error = proposed_h - current_h
        if math.log(rng.uniform()) < min(0.0, -energy_error):
            q = proposed_q
            if iteration >= warmup: accepted += 1
        if iteration >= warmup:
            chain.append(q); energy_errors.append(energy_error)
    values = np.asarray(chain)
    return {
        "format":"marklab.fixed_step_hmc_normal_mean", "version":1,
        "algorithm":{"mass":mass,"step_size":step,"leapfrog_steps":leapfrog_steps,"warmup":warmup,"draws":draws,"seed":spec["seed"]},
        "posterior":{"mean":float(values.mean()),"standard_deviation":float(values.std(ddof=1)),"interval_95":np.quantile(values,[0.025,0.975]).tolist(),"draws":values.tolist()},
        "analytic_posterior":{"mean":analytic_mean,"standard_deviation":math.sqrt(analytic_variance)},
        "diagnostics":{"acceptance_rate":accepted/draws,"maximum_absolute_energy_error":float(np.max(np.abs(energy_errors))),"mean_energy_error":float(np.mean(energy_errors)),"constraint_transform_jacobian":0.0},
        "claim_status":"experimental_synthetic_fixed_step_hmc",
    }


def cluster_scores(points, maximum):
    results = []
    for count in range(1, maximum + 1):
        order = np.argsort(points[:, 0] + 0.01 * points[:, 1])
        initial = points[order[np.linspace(0, len(points) - 1, count).astype(int)]]
        centers, labels = kmeans2(points, initial, minit="matrix", iter=50)
        residual = points - centers[labels]
        rss = max(float(np.sum(residual * residual)), 1e-12)
        variance = rss / (2 * len(points))
        # Two coordinates per parent, mixture weights, and one shared scale.
        bic = len(points) * 2 * math.log(variance) + (4 * count) * math.log(len(points))
        results.append((count, bic, math.sqrt(variance), centers, labels))
    return results


def sample_parent_count(scores, iterations, burnin, rng):
    by_count = {row[0]: row for row in scores}; current = min(scores, key=lambda row: row[1])[0]
    retained = []; accepted = 0
    for iteration in range(iterations):
        direction = -1 if rng.uniform() < 0.5 else 1
        proposal = current + direction
        if proposal in by_count:
            log_alpha = -0.5 * (by_count[proposal][1] - by_count[current][1])
            if math.log(rng.uniform()) < min(0.0, log_alpha):
                current = proposal; accepted += 1
        if iteration >= burnin: retained.append(current)
    return np.asarray(retained), accepted / iterations


def exact_exchange(spec, rng):
    sites = int(spec["sites"]); edges = [tuple(edge) for edge in spec["edges"]]
    observed = tuple(int(value) for value in spec["observed_types"])
    if not 2 <= sites <= 16 or len(observed) != sites or set(observed) != {0, 1} or any(not (0 <= a < b < sites) for a, b in edges):
        raise ContractError("finite binary Gibbs contract violated")
    states = list(itertools.product([0, 1], repeat=sites))
    statistics = np.asarray([sum(state[a] == state[b] for a, b in edges) for state in states], float)
    observed_stat = float(sum(observed[a] == observed[b] for a, b in edges))
    prior_sd = float(spec["interaction_prior_sd"]); proposal_sd = float(spec["proposal_sd"])
    draws = int(spec["draws"]); burnin = int(spec["burnin"])
    theta = 0.0; retained = []; accepted = 0
    for iteration in range(draws + burnin):
        proposal = theta + rng.normal(0.0, proposal_sd)
        weights = np.exp(proposal * statistics - np.max(proposal * statistics)); weights /= weights.sum()
        auxiliary_stat = statistics[rng.choice(len(states), p=weights)]
        log_alpha = -0.5 * (proposal**2 - theta**2) / prior_sd**2 + (proposal - theta) * observed_stat + (theta - proposal) * auxiliary_stat
        if math.log(rng.uniform()) < min(0.0, log_alpha):
            theta = proposal
            if iteration >= burnin: accepted += 1
        if iteration >= burnin: retained.append(theta)
    values = np.asarray(retained)
    return {"posterior_interaction_mean":float(values.mean()),"posterior_interaction_interval_95":np.quantile(values,[0.025,0.975]).tolist(),"acceptance_rate":accepted/draws,"observed_same_type_edge_statistic":observed_stat,"state_space_size":len(states),"auxiliary_draw_status":"exact_finite_state_enumeration","draws":values.tolist()}


def advanced_cluster(spec):
    patterns = spec["patterns"]
    if spec["cluster_family"] != "thomas_gaussian" or not 3 <= len(patterns) <= 64 or len({p["pattern_id"] for p in patterns}) != len(patterns) or len({p["patient_id"] for p in patterns}) != len(patterns):
        raise ContractError("independent unique Thomas patterns required")
    maximum = int(spec["maximum_parents"]); iterations = int(spec["latent_iterations"]); burnin = int(spec["latent_burnin"])
    if not 2 <= maximum <= 16 or not 500 <= iterations <= 20000 or not 100 <= burnin < iterations:
        raise ContractError("latent-parent bounds violated")
    rng = np.random.default_rng(spec["seed"]); pattern_results = []
    for pattern in patterns:
        points = np.asarray(pattern["points"], float); window = pattern["window"]
        if points.ndim != 2 or points.shape[1] != 2 or len(points) < 12 or not np.isfinite(points).all() or not all(window[0] <= point[0] <= window[1] and window[2] <= point[1] <= window[3] for point in points):
            raise ContractError("bounded finite cluster points required")
        scores = cluster_scores(points, maximum); counts, acceptance = sample_parent_count(scores, iterations, burnin, rng)
        chosen = min(scores, key=lambda row: abs(row[0] - counts.mean()))
        area = (window[1] - window[0]) * (window[3] - window[2])
        pattern_results.append({"pattern_id":pattern["pattern_id"],"patient_id":pattern["patient_id"],"parent_count_mean":float(counts.mean()),"parent_count_interval_95":np.quantile(counts,[0.025,0.975]).tolist(),"scale_mean":chosen[2],"kappa":float(counts.mean()/area),"mu":float(len(points)/counts.mean()),"birth_death_acceptance_rate":acceptance,"boundary_policy":"exact_rectangular_observation_window_no_parent_edge_correction"})
    exchange = exact_exchange(spec["finite_gibbs"], rng)
    shrinkage = float(spec["hierarchical_shrinkage"])
    raw = np.asarray([[math.log(row["kappa"]),math.log(row["mu"]),math.log(row["scale_mean"])] for row in pattern_results])
    center = raw.mean(axis=0); pooled = (len(patterns) * raw + shrinkage * center) / (len(patterns) + shrinkage)
    aggregate_parent = float(np.mean([row["parent_count_mean"] for row in pattern_results]))
    aggregate_scale = float(np.mean([row["scale_mean"] for row in pattern_results]))
    return {
        "format":"marklab.advanced_cluster_and_gibbs_models","version":1,
        "latent_parent_model":{"family":"thomas_gaussian","inference":"label_invariant_birth_death_parent_count_plus_conditional_kmeans","posterior_parent_count_mean":aggregate_parent,"posterior_scale_mean":aggregate_scale,"pattern_results":pattern_results,"posterior_predictive_checks":{"component_size_mean":float(np.mean([len(p["points"])/r["parent_count_mean"] for p,r in zip(patterns,pattern_results)])),"window_edge_status":"reported_not_corrected"}},
        "exchange_mcmc":exchange,
        "multitype_gibbs":{"types":[0,1],"symmetric_interaction":True,"method":"exact_finite_state_exchange_mcmc","posterior_interaction_mean":exchange["posterior_interaction_mean"],"multiplicity_policy":"single_prespecified_type_pair","posterior_predictive_checks":["type_frequency","same_type_edges"]},
        "replicated_cluster_model":{"patient_count":len(patterns),"partial_pooling_applied":True,"population_log_parameter_mean":center.tolist(),"patient_log_parameter_means":pooled.tolist(),"parameters":["log_kappa","log_mu","log_scale"]},
        "claim_status":"experimental_synthetic_cluster_and_finite_gibbs_inference",
    }


def build_mesh(bounds, resolution, kappa, tau):
    x0, x1, y0, y1 = map(float, bounds)
    xs = np.linspace(x0, x1, resolution); ys = np.linspace(y0, y1, resolution)
    vertices = np.asarray([[x, y] for y in ys for x in xs], dtype=float)
    triangulation = Delaunay(vertices)
    triangles = np.asarray(sorted((tuple(sorted(map(int, simplex))) for simplex in triangulation.simplices)), dtype=int)
    mass = np.zeros((len(vertices), len(vertices))); stiffness = np.zeros_like(mass)
    areas = []
    for triangle in triangles:
        coordinates = vertices[triangle]
        signed_double_area = np.cross(coordinates[1] - coordinates[0], coordinates[2] - coordinates[0])
        area = abs(float(signed_double_area)) / 2.0
        if area <= 0:
            raise ContractError("degenerate SPDE triangle")
        areas.append(area)
        local_mass = area / 12.0 * np.asarray([[2,1,1],[1,2,1],[1,1,2]], float)
        b = np.asarray([coordinates[1,1]-coordinates[2,1],coordinates[2,1]-coordinates[0,1],coordinates[0,1]-coordinates[1,1]]) / (2.0 * area)
        c = np.asarray([coordinates[2,0]-coordinates[1,0],coordinates[0,0]-coordinates[2,0],coordinates[1,0]-coordinates[0,0]]) / (2.0 * area)
        local_stiffness = area * (np.outer(b,b) + np.outer(c,c))
        for local_i, global_i in enumerate(triangle):
            for local_j, global_j in enumerate(triangle):
                mass[global_i,global_j] += local_mass[local_i,local_j]
                stiffness[global_i,global_j] += local_stiffness[local_i,local_j]
    lumped = mass.sum(axis=1)
    precision = tau**2 * (kappa**4 * mass + 2.0 * kappa**2 * stiffness + stiffness @ np.diag(1.0/lumped) @ stiffness)
    precision = (precision + precision.T) / 2.0
    return {"vertices":vertices,"triangles":triangles,"triangulation":triangulation,"mass":mass,"stiffness":stiffness,"precision":precision,"triangle_areas":np.asarray(areas)}


def projection(mesh, locations):
    locations = np.asarray(locations, float)
    triangulation = mesh["triangulation"]
    simplices = triangulation.find_simplex(locations)
    if np.any(simplices < 0):
        raise ContractError("SPDE projection location lies outside the rectangular mesh")
    matrix = np.zeros((len(locations), len(mesh["vertices"])))
    for row, (point, simplex_index) in enumerate(zip(locations, simplices)):
        transform = triangulation.transform[simplex_index]
        first = transform[:2] @ (point - transform[2])
        barycentric = np.asarray([first[0], first[1], 1.0-first.sum()])
        if barycentric.min() < -1e-10:
            raise ContractError("invalid SPDE barycentric projection")
        matrix[row,triangulation.simplices[simplex_index]] = barycentric
    return matrix


def fit_lgcp(mesh, points, prior_sd, maximum_iterations):
    event_projection = projection(mesh, points)
    triangle_coordinates = mesh["vertices"][mesh["triangles"]]
    quadrature_locations = triangle_coordinates.mean(axis=1)
    quadrature_projection = projection(mesh, quadrature_locations)
    weights = mesh["triangle_areas"]
    precision = mesh["precision"]; count = len(points)
    def objective(parameter):
        beta = parameter[0]; field = parameter[1:]
        event_eta = beta + event_projection @ field
        quad_eta = beta + quadrature_projection @ field
        intensity = np.exp(np.clip(quad_eta, -30.0, 30.0))
        value = 0.5*beta**2/prior_sd**2 + 0.5*field@precision@field - event_eta.sum() + weights@intensity
        gradient = np.concatenate([[beta/prior_sd**2-count+weights@intensity], precision@field-event_projection.sum(axis=0)+quadrature_projection.T@(weights*intensity)])
        return float(value), gradient
    initial = np.zeros(len(mesh["vertices"])+1); initial[0] = math.log(count/np.sum(weights))
    fit = minimize(objective, initial, jac=True, method="L-BFGS-B", options={"maxiter":maximum_iterations,"ftol":1e-12,"gtol":1e-7})
    if not fit.success and np.linalg.norm(fit.jac, ord=np.inf) > 2e-4:
        raise ContractError(f"SPDE LGCP optimization failed: {fit.message}")
    beta = float(fit.x[0]); field = fit.x[1:]
    vertex_intensity = np.exp(np.clip(beta+field,-30,30))
    quad_intensity = np.exp(np.clip(beta+quadrature_projection@field,-30,30))
    midpoint = (mesh["vertices"][:,0].min()+mesh["vertices"][:,0].max())/2.0
    left = vertex_intensity[mesh["vertices"][:,0] < midpoint].mean()
    right = vertex_intensity[mesh["vertices"][:,0] > midpoint].mean()
    return {"beta":beta,"field":field,"vertex_intensity":vertex_intensity,"integrated":float(weights@quad_intensity),"right_left_ratio":float(right/left),"objective":float(fit.fun),"gradient_max":float(np.linalg.norm(fit.jac,ord=np.inf)),"iterations":int(fit.nit),"event_projection":event_projection,"quadrature_projection":quadrature_projection,"quadrature_weights":weights}


def fit_spatial_factor(mesh, observations, noise_sd, maximum_iterations):
    coordinates = np.asarray([row["coordinates"] for row in observations], float)
    values = np.asarray([row["values"] for row in observations], float)
    if values.ndim != 2 or values.shape[1] < 2 or not np.isfinite(values).all():
        raise ContractError("SPDE factor model requires a finite matrix with at least two features")
    design = projection(mesh, coordinates); means = values.mean(axis=0); centered = values-means
    initial_field = np.linalg.solve(design.T@design + noise_sd**2*mesh["precision"] + np.eye(len(mesh["vertices"]))*1e-8, design.T@centered[:,0])
    initial_loadings = np.linalg.lstsq((design@initial_field)[:,None], centered, rcond=None)[0].ravel()
    initial = np.concatenate([initial_field,initial_loadings])
    vertex_count = len(initial_field); precision = mesh["precision"]
    def objective(parameter):
        field = parameter[:vertex_count]; loadings = parameter[vertex_count:]
        projected = design@field; residual = centered-projected[:,None]*loadings[None,:]
        value = 0.5*np.sum(residual**2)/noise_sd**2 + 0.5*field@precision@field + 0.5*loadings@loadings
        gradient_field = precision@field-design.T@(residual@loadings)/noise_sd**2
        gradient_loadings = loadings-projected@residual/noise_sd**2
        return float(value),np.concatenate([gradient_field,gradient_loadings])
    fit = minimize(objective,initial,jac=True,method="L-BFGS-B",options={"maxiter":maximum_iterations,"ftol":1e-12,"gtol":1e-7})
    if not fit.success and np.linalg.norm(fit.jac,ord=np.inf)>2e-4:
        raise ContractError(f"SPDE factor optimization failed: {fit.message}")
    field=fit.x[:vertex_count]; loadings=fit.x[vertex_count:]; projected=design@field
    if loadings[0]<0: field=-field; loadings=-loadings; projected=-projected
    reconstruction=means+projected[:,None]*loadings[None,:]
    correlation=float(np.corrcoef(projected,coordinates[:,0])[0,1])
    return {"means":means,"field":field,"projected":projected,"loadings":loadings,"rmse":float(np.sqrt(np.mean((reconstruction-values)**2))),"x_correlation":correlation,"objective":float(fit.fun),"iterations":int(fit.nit),"projection":design}


def sparse_triplets(matrix, tolerance=1e-14):
    rows,columns=np.nonzero(np.abs(matrix)>tolerance)
    return {"rows":rows.tolist(),"columns":columns.tolist(),"values":matrix[rows,columns].tolist(),"shape":list(matrix.shape)}


def signed_ring_area(ring):
    return 0.5 * sum(
        ring[index][0] * ring[index + 1][1] - ring[index + 1][0] * ring[index][1]
        for index in range(len(ring) - 1)
    )


def parse_polygonal_window(value):
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
            if ring.ndim != 2 or ring.shape[1] != 2 or len(ring) < 4 or not np.isfinite(ring).all() or not np.array_equal(ring[0], ring[-1]):
                raise ContractError("adaptive SPDE rings must be finite, closed, and have at least four positions")
            area = abs(float(signed_ring_area(ring)))
            if area <= 0.0:
                raise ContractError("adaptive SPDE ring has zero area")
            rings.append(ring)
            vertex_count += len(ring)
            boundary_segments.extend((ring[index], ring[index + 1]) for index in range(len(ring) - 1))
        polygon_area = abs(float(signed_ring_area(rings[0]))) - sum(abs(float(signed_ring_area(ring))) for ring in rings[1:])
        if polygon_area <= 0.0:
            raise ContractError("adaptive SPDE polygon holes consume its exterior")
        total_area += polygon_area
        polygons.append(rings)
    if not polygons or vertex_count > 100000:
        raise ContractError("adaptive SPDE window component/vertex bound violated")
    all_points = np.vstack([ring for polygon in polygons for ring in polygon])
    bounds = [float(all_points[:, 0].min()), float(all_points[:, 0].max()), float(all_points[:, 1].min()), float(all_points[:, 1].max())]
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
    inside = False
    x, y = map(float, point)
    for first, second in zip(ring[:-1], ring[1:]):
        if point_on_segment(np.asarray(point), first, second):
            return True, True
        y_crosses = (first[1] > y) != (second[1] > y)
        if y_crosses:
            crossing_x = first[0] + (y - first[1]) * (second[0] - first[0]) / (second[1] - first[1])
            if crossing_x > x:
                inside = not inside
    return inside, False


def window_contains(window, point):
    for polygon in window["polygons"]:
        exterior, exterior_boundary = point_in_ring(point, polygon[0])
        if not exterior:
            continue
        if exterior_boundary:
            return True
        excluded = False
        for hole in polygon[1:]:
            in_hole, hole_boundary = point_in_ring(point, hole)
            if hole_boundary:
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


def segment_within_window(window, first, second):
    def orientation(a, b, c):
        return float((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
    for boundary_first, boundary_second in window["segments"]:
        first_side = orientation(first, second, boundary_first)
        second_side = orientation(first, second, boundary_second)
        boundary_first_side = orientation(boundary_first, boundary_second, first)
        boundary_second_side = orientation(boundary_first, boundary_second, second)
        tolerance = 1e-12 * max(1.0, float(np.linalg.norm(second - first)), float(np.linalg.norm(boundary_second - boundary_first)))
        if first_side * second_side < -(tolerance ** 2) and boundary_first_side * boundary_second_side < -(tolerance ** 2):
            return False
    return all(window_contains(window, first + fraction * (second - first)) for fraction in np.linspace(0.0, 1.0, 17))


def build_adaptive_window_mesh(window, base_resolution, refinement_level, observation_coordinates, kappa, tau, maximum_vertices, maximum_triangles, maximum_boundary_checks, memory_budget_bytes):
    x0, x1, y0, y1 = window["bounds"]
    xs = np.linspace(x0, x1, base_resolution)
    ys = np.linspace(y0, y1, base_resolution)
    base_spacing = min((x1 - x0) / (base_resolution - 1), (y1 - y0) / (base_resolution - 1))
    boundary_spacing = base_spacing / (2 ** refinement_level)
    boundary_point_count = sum(max(1, int(math.ceil(float(np.linalg.norm(second - first)) / boundary_spacing))) for first, second in window["segments"])
    if boundary_point_count > 4 * maximum_vertices:
        raise ContractError("adaptive SPDE boundary refinement exceeds the bounded candidate budget")
    candidates = [[x, y] for y in ys for x in xs if window_contains(window, np.asarray([x, y]))]
    candidates.extend(resampled_boundary_points(window, boundary_spacing))
    candidates.extend(observation_coordinates.tolist())
    canonical = sorted({(float(point[0]), float(point[1])) for point in candidates})
    vertices = np.asarray(canonical, dtype=float)
    if len(vertices) < 3 or len(vertices) > maximum_vertices:
        raise ContractError("adaptive SPDE vertex bound violated")
    triangulation = Delaunay(vertices)
    boundary_checks = len(triangulation.simplices) * 3 * len(window["segments"])
    if boundary_checks > maximum_boundary_checks:
        raise ContractError("adaptive SPDE boundary-segment/triangle work bound exceeded")
    retained = []
    for simplex in triangulation.simplices:
        triangle = tuple(sorted(map(int, simplex)))
        coordinates = vertices[list(triangle)]
        centroid = coordinates.mean(axis=0)
        if not window_contains(window, centroid):
            continue
        if all(segment_within_window(window, coordinates[index], coordinates[(index + 1) % 3]) for index in range(3)):
            retained.append(triangle)
    triangles = np.asarray(sorted(set(retained)), dtype=int)
    if len(triangles) == 0 or len(triangles) > maximum_triangles:
        raise ContractError("adaptive SPDE triangle bound violated")
    used = sorted(set(map(int, triangles.ravel())))
    remap = {old: new for new, old in enumerate(used)}
    vertices = vertices[used]
    triangles = np.asarray([[remap[int(index)] for index in triangle] for triangle in triangles], dtype=int)
    vertex_count = len(vertices)
    dense_bytes = 10 * vertex_count * vertex_count * 8
    if dense_bytes > memory_budget_bytes:
        raise ContractError("adaptive SPDE dense finite-element memory budget exceeded")
    mass = np.zeros((vertex_count, vertex_count))
    stiffness = np.zeros_like(mass)
    areas = []
    for triangle in triangles:
        coordinates = vertices[triangle]
        signed_double_area = float(np.cross(coordinates[1] - coordinates[0], coordinates[2] - coordinates[0]))
        area = abs(signed_double_area) / 2.0
        if area <= 1e-14:
            raise ContractError("adaptive SPDE produced a degenerate triangle")
        areas.append(area)
        local_mass = area / 12.0 * np.asarray([[2, 1, 1], [1, 2, 1], [1, 1, 2]], float)
        denominator = signed_double_area
        b = np.asarray([coordinates[1, 1] - coordinates[2, 1], coordinates[2, 1] - coordinates[0, 1], coordinates[0, 1] - coordinates[1, 1]]) / denominator
        c = np.asarray([coordinates[2, 0] - coordinates[1, 0], coordinates[0, 0] - coordinates[2, 0], coordinates[1, 0] - coordinates[0, 0]]) / denominator
        local_stiffness = area * (np.outer(b, b) + np.outer(c, c))
        for local_i, global_i in enumerate(triangle):
            for local_j, global_j in enumerate(triangle):
                mass[global_i, global_j] += local_mass[local_i, local_j]
                stiffness[global_i, global_j] += local_stiffness[local_i, local_j]
    lumped = mass.sum(axis=1)
    if np.any(lumped <= 0.0):
        raise ContractError("adaptive SPDE mesh contains an unused or zero-mass vertex")
    precision = tau ** 2 * (kappa ** 4 * mass + 2.0 * kappa ** 2 * stiffness + stiffness @ np.diag(1.0 / lumped) @ stiffness)
    precision = (precision + precision.T) / 2.0
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
    return {"vertices": vertices, "triangles": triangles, "mass": mass, "stiffness": stiffness, "precision": precision, "triangle_areas": np.asarray(areas), "mesh_area": float(sum(areas)), "connected_components": components, "boundary_spacing": boundary_spacing}


def adaptive_projection(mesh, locations, maximum_visits):
    locations = np.asarray(locations, dtype=float)
    matrix = np.zeros((len(locations), len(mesh["vertices"])))
    visits = 0
    for row, point in enumerate(locations):
        found = False
        for triangle in mesh["triangles"]:
            visits += 1
            if visits > maximum_visits:
                raise ContractError("adaptive SPDE projection triangle-visit bound exceeded")
            coordinates = mesh["vertices"][triangle]
            denominator = ((coordinates[1, 1] - coordinates[2, 1]) * (coordinates[0, 0] - coordinates[2, 0]) + (coordinates[2, 0] - coordinates[1, 0]) * (coordinates[0, 1] - coordinates[2, 1]))
            if abs(float(denominator)) <= 1e-14:
                continue
            first = ((coordinates[1, 1] - coordinates[2, 1]) * (point[0] - coordinates[2, 0]) + (coordinates[2, 0] - coordinates[1, 0]) * (point[1] - coordinates[2, 1])) / denominator
            second = ((coordinates[2, 1] - coordinates[0, 1]) * (point[0] - coordinates[2, 0]) + (coordinates[0, 0] - coordinates[2, 0]) * (point[1] - coordinates[2, 1])) / denominator
            barycentric = np.asarray([first, second, 1.0 - first - second])
            if barycentric.min() >= -1e-10 and barycentric.max() <= 1.0 + 1e-10:
                matrix[row, triangle] = np.clip(barycentric, 0.0, 1.0)
                matrix[row] /= matrix[row].sum()
                found = True
                break
        if not found:
            raise ContractError("adaptive SPDE observation is not covered by the retained mesh")
    return matrix, visits


def fit_adaptive_spatial_factor(mesh, observations, noise_sd, maximum_iterations, maximum_projection_visits):
    coordinates = np.asarray([row["coordinates"] for row in observations], dtype=float)
    values = np.asarray([row["values"] for row in observations], dtype=float)
    design, projection_visits = adaptive_projection(mesh, coordinates, maximum_projection_visits)
    means = values.mean(axis=0)
    centered = values - means
    initial_field = np.linalg.solve(design.T @ design + noise_sd ** 2 * mesh["precision"] + np.eye(len(mesh["vertices"])) * 1e-8, design.T @ centered[:, 0])
    initial_loadings = np.linalg.lstsq((design @ initial_field)[:, None], centered, rcond=None)[0].ravel()
    if not np.isfinite(initial_loadings).all() or float(initial_loadings @ initial_loadings) <= 1e-16:
        initial_loadings = np.zeros(values.shape[1]); initial_loadings[0] = 1.0
    field = initial_field
    loadings = initial_loadings
    precision = mesh["precision"]
    gram = design.T @ design
    converged = False
    objective_value = math.inf
    gradient_maximum = math.inf
    block_iterations = min(50, max(1, maximum_iterations // 5))
    for iteration in range(1, block_iterations + 1):
        loading_norm = float(loadings @ loadings)
        system = precision + (loading_norm / noise_sd ** 2) * gram + np.eye(len(field)) * 1e-10
        right_hand_side = design.T @ (centered @ loadings) / noise_sd ** 2
        field = np.linalg.solve(system, right_hand_side)
        projected = design @ field
        denominator = 1.0 + float(projected @ projected) / noise_sd ** 2
        loadings = (projected @ centered) / (noise_sd ** 2 * denominator)
        residual = centered - projected[:, None] * loadings[None, :]
        objective_value = float(0.5 * np.sum(residual ** 2) / noise_sd ** 2 + 0.5 * field @ precision @ field + 0.5 * loadings @ loadings)
        gradient_field = precision @ field - design.T @ (residual @ loadings) / noise_sd ** 2
        gradient_loadings = loadings - projected @ residual / noise_sd ** 2
        gradient_maximum = float(np.linalg.norm(np.concatenate([gradient_field, gradient_loadings]), ord=np.inf))
        if not math.isfinite(objective_value) or not math.isfinite(gradient_maximum):
            raise ContractError("adaptive SPDE block-coordinate solver produced a nonfinite state")
        if gradient_maximum <= 2e-4:
            converged = True
            break
    if not converged:
        vertex_count = len(field)
        def objective(parameter):
            candidate_field = parameter[:vertex_count]
            candidate_loadings = parameter[vertex_count:]
            candidate_projected = design @ candidate_field
            candidate_residual = centered - candidate_projected[:, None] * candidate_loadings[None, :]
            value = 0.5 * np.sum(candidate_residual ** 2) / noise_sd ** 2 + 0.5 * candidate_field @ precision @ candidate_field + 0.5 * candidate_loadings @ candidate_loadings
            gradient_field = precision @ candidate_field - design.T @ (candidate_residual @ candidate_loadings) / noise_sd ** 2
            gradient_loadings = candidate_loadings - candidate_projected @ candidate_residual / noise_sd ** 2
            return float(value), np.concatenate([gradient_field, gradient_loadings])
        fit = minimize(objective, np.concatenate([field, loadings]), jac=True, method="L-BFGS-B", options={"maxiter": maximum_iterations - block_iterations, "ftol": 1e-12, "gtol": 1e-7})
        field = fit.x[:vertex_count]
        loadings = fit.x[vertex_count:]
        objective_value = float(fit.fun)
        gradient_maximum = float(np.linalg.norm(fit.jac, ord=np.inf))
        iteration = block_iterations + int(fit.nit)
        if not math.isfinite(objective_value) or not math.isfinite(gradient_maximum) or (not fit.success and gradient_maximum > 2e-4):
            raise ContractError(f"adaptive SPDE warmed optimization failed after {iteration} iterations with gradient {gradient_maximum}: {fit.message}")
    projected = design @ field
    if loadings[0] < 0:
        field = -field
        loadings = -loadings
        projected = -projected
    reconstruction = means + projected[:, None] * loadings[None, :]
    correlation = float(np.corrcoef(projected, coordinates[:, 0])[0, 1])
    return {"means": means, "field": field, "projected": projected, "loadings": loadings, "rmse": float(np.sqrt(np.mean((reconstruction - values) ** 2))), "x_correlation": correlation, "objective": objective_value, "gradient_max": gradient_maximum, "iterations": iteration, "projection": design, "projection_visits": projection_visits}


def anisotropy_tensor(ratio, major_axis_degrees):
    ratio = float(ratio)
    angle = float(major_axis_degrees)
    if not math.isfinite(ratio) or not math.isfinite(angle) or not 1.0 <= ratio <= 16.0 or not -180.0 <= angle <= 180.0:
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
    matched = [region for region in regions if float(np.linalg.norm(point - region["center"])) <= region["radius"] + 1e-12]
    if len(matched) > 1:
        raise ContractError("nonstationary SPDE parameter regions overlap")
    return matched[0] if matched else background


def build_nonstationary_adaptive_mesh(window, spec, observation_coordinates):
    x0, x1, y0, y1 = window["bounds"]
    base_resolution = int(spec["base_resolution"])
    boundary_level = int(spec["boundary_refinement_levels"])
    maximum_vertices = int(spec["maximum_vertices"])
    maximum_triangles = int(spec["maximum_triangles"])
    maximum_candidates = int(spec["maximum_candidate_points"])
    maximum_boundary_checks = int(spec["maximum_boundary_segment_triangle_checks"])
    memory_budget_bytes = int(spec["memory_budget_mib"]) * 1024 * 1024
    base_spacing = min((x1 - x0) / (base_resolution - 1), (y1 - y0) / (base_resolution - 1))
    boundary_spacing = base_spacing / (2 ** boundary_level)
    xs = np.linspace(x0, x1, base_resolution)
    ys = np.linspace(y0, y1, base_resolution)
    candidates = [[x, y] for y in ys for x in xs if window_contains(window, np.asarray([x, y]))]
    candidates.extend(resampled_boundary_points(window, boundary_spacing))
    candidates.extend(observation_coordinates.tolist())
    background_raw = spec["background"]
    background = {
        "region_id": "background",
        "center": None,
        "radius": None,
        "kappa": float(background_raw["kappa"]),
        "tau": float(background_raw["tau"]),
        "anisotropy_ratio": float(background_raw["anisotropy_ratio"]),
        "major_axis_degrees": float(background_raw["major_axis_degrees"]),
        "interior_refinement_levels": 0,
    }
    regions = []
    identities = set()
    for raw in spec["parameter_regions"]:
        identity = raw["region_id"]
        center = np.asarray(raw["center"], dtype=float)
        radius = float(raw["radius"])
        level = int(raw["interior_refinement_levels"])
        region = {
            "region_id": identity,
            "center": center,
            "radius": radius,
            "kappa": float(raw["kappa"]),
            "tau": float(raw["tau"]),
            "anisotropy_ratio": float(raw["anisotropy_ratio"]),
            "major_axis_degrees": float(raw["major_axis_degrees"]),
            "interior_refinement_levels": level,
        }
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
        for second in regions[left + 1:]:
            if float(np.linalg.norm(first["center"] - second["center"])) <= first["radius"] + second["radius"]:
                raise ContractError("nonstationary SPDE parameter regions overlap")
    for parameter in [background, *regions]:
        if not all(math.isfinite(parameter[key]) and parameter[key] > 0.0 for key in ("kappa", "tau")):
            raise ContractError("nonstationary SPDE kappa and tau must be finite and positive")
        parameter["anisotropy_tensor"] = anisotropy_tensor(
            parameter["anisotropy_ratio"], parameter["major_axis_degrees"]
        )
    for region in regions:
        spacing = base_spacing / (2 ** region["interior_refinement_levels"])
        count = int(math.ceil(2.0 * region["radius"] / spacing))
        for row in range(count + 1):
            y = region["center"][1] - region["radius"] + row * spacing
            for column in range(count + 1):
                x = region["center"][0] - region["radius"] + column * spacing
                point = np.asarray([x, y])
                if float(np.linalg.norm(point - region["center"])) <= region["radius"] + 1e-12 and window_contains(window, point):
                    candidates.append([x, y])
                if len(candidates) > maximum_candidates:
                    raise ContractError("nonstationary SPDE adaptive candidate bound exceeded")
    canonical = sorted({(float(point[0]), float(point[1])) for point in candidates})
    if len(canonical) > maximum_candidates:
        raise ContractError("nonstationary SPDE unique candidate bound exceeded")
    vertices = np.asarray(canonical, dtype=float)
    if len(vertices) < 3 or len(vertices) > maximum_vertices:
        raise ContractError("nonstationary SPDE vertex bound violated")
    triangulation = Delaunay(vertices)
    boundary_checks = len(triangulation.simplices) * 3 * len(window["segments"])
    if boundary_checks > maximum_boundary_checks:
        raise ContractError("nonstationary SPDE boundary work bound exceeded")
    retained = []
    for simplex in triangulation.simplices:
        triangle = tuple(sorted(map(int, simplex)))
        coordinates = vertices[list(triangle)]
        if not window_contains(window, coordinates.mean(axis=0)):
            continue
        if all(segment_within_window(window, coordinates[index], coordinates[(index + 1) % 3]) for index in range(3)):
            retained.append(triangle)
    triangles = np.asarray(sorted(set(retained)), dtype=int)
    if len(triangles) == 0 or len(triangles) > maximum_triangles:
        raise ContractError("nonstationary SPDE triangle bound violated")
    used = sorted(set(map(int, triangles.ravel())))
    remap = {old: new for new, old in enumerate(used)}
    vertices = vertices[used]
    triangles = np.asarray([[remap[int(index)] for index in triangle] for triangle in triangles], dtype=int)
    vertex_count = len(vertices)
    if 12 * vertex_count * vertex_count * 8 > memory_budget_bytes:
        raise ContractError("nonstationary SPDE dense FEM memory budget exceeded")
    vertex_parameters = [nonstationary_parameter_at(point, background, regions) for point in vertices]
    mass = np.zeros((vertex_count, vertex_count))
    stiffness = np.zeros_like(mass)
    areas = []
    triangle_regions = []
    for triangle in triangles:
        coordinates = vertices[triangle]
        signed_double_area = float(np.cross(coordinates[1] - coordinates[0], coordinates[2] - coordinates[0]))
        area = abs(signed_double_area) / 2.0
        if area <= 1e-14:
            raise ContractError("nonstationary SPDE produced a degenerate triangle")
        areas.append(area)
        triangle_regions.append(nonstationary_parameter_at(coordinates.mean(axis=0), background, regions)["region_id"])
        local_mass = area / 12.0 * np.asarray([[2, 1, 1], [1, 2, 1], [1, 1, 2]], float)
        denominator = signed_double_area
        b = np.asarray([coordinates[1, 1] - coordinates[2, 1], coordinates[2, 1] - coordinates[0, 1], coordinates[0, 1] - coordinates[1, 1]]) / denominator
        c = np.asarray([coordinates[2, 0] - coordinates[1, 0], coordinates[0, 0] - coordinates[2, 0], coordinates[1, 0] - coordinates[0, 0]]) / denominator
        gradients = np.column_stack([b, c])
        tensor = sum((vertex_parameters[int(index)]["anisotropy_tensor"] for index in triangle), np.zeros((2, 2))) / 3.0
        local_stiffness = area * gradients @ tensor @ gradients.T
        for local_i, global_i in enumerate(triangle):
            for local_j, global_j in enumerate(triangle):
                mass[global_i, global_j] += local_mass[local_i, local_j]
                stiffness[global_i, global_j] += local_stiffness[local_i, local_j]
    areas = np.asarray(areas)
    lumped = mass.sum(axis=1)
    if np.any(lumped <= 0.0):
        raise ContractError("nonstationary SPDE mesh has zero mass")
    kappas = np.asarray([parameter["kappa"] for parameter in vertex_parameters])
    taus = np.asarray([parameter["tau"] for parameter in vertex_parameters])
    operator = stiffness + np.diag(lumped * kappas ** 2)
    weighted = operator @ np.diag(taus)
    precision = weighted.T @ np.diag(1.0 / lumped) @ weighted
    precision = (precision + precision.T) / 2.0
    symmetry_error = float(np.max(np.abs(precision - precision.T)))
    eigenvalues = np.linalg.eigvalsh(precision)
    if eigenvalues[0] <= 0.0:
        raise ContractError("nonstationary SPDE precision is not positive definite")
    summaries = []
    for parameter in [background, *regions]:
        selected = areas[np.asarray(triangle_regions) == parameter["region_id"]]
        if len(selected) == 0:
            raise ContractError(f"nonstationary SPDE region {parameter['region_id']} has no retained triangles")
        summaries.append({
            "region_id": parameter["region_id"],
            "kappa": parameter["kappa"],
            "tau": parameter["tau"],
            "anisotropy_ratio": parameter["anisotropy_ratio"],
            "major_axis_degrees": parameter["major_axis_degrees"],
            "anisotropy_tensor": parameter["anisotropy_tensor"].tolist(),
            "interior_refinement_levels": parameter["interior_refinement_levels"],
            "triangle_count": len(selected),
            "median_triangle_area": float(np.median(selected)),
        })
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
        or not 1024 <= maximum_result_bytes <= 16 * 1024 * 1024
    ):
        raise ContractError("nonstationary adaptive SPDE controls violate their bounds")
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
        raise ContractError("nonstationary SPDE observations are invalid")
    mesh = build_nonstationary_adaptive_mesh(window, spec, coordinates)
    relative_area_error = abs(mesh["mesh_area"] - window["exact_area"]) / window["exact_area"]
    if relative_area_error > maximum_relative_area_error:
        raise ContractError(
            f"nonstationary SPDE mesh relative area error {relative_area_error} exceeds {maximum_relative_area_error}"
        )
    factor = fit_adaptive_spatial_factor(
        mesh, observations, noise_sd, maximum_iterations, maximum_projection_visits
    )
    projection_error = float(np.max(np.abs(factor["projection"].sum(axis=1) - 1.0)))
    result = {
        "format": "marklab.nonstationary_adaptive_window_spde",
        "version": 1,
        "statistical_unit": "within_specimen_field_diagnostic",
        "window": {
            "frame": frame,
            "exact_area": window["exact_area"],
            "bounds": window["bounds"],
            "component_count": window["component_count"],
            "hole_count": window["hole_count"],
            "ring_count": window["ring_count"],
            "vertex_count": window["vertex_count"],
        },
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
            "maximum_boundary_segment_triangle_checks": int(spec["maximum_boundary_segment_triangle_checks"]),
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
    if len(json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True).encode()) > maximum_result_bytes:
        raise ContractError("nonstationary SPDE result exceeds maximum_result_bytes")
    return result


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
    memory_budget_bytes = int(spec["memory_budget_mib"]) * 1024 * 1024
    maximum_result_bytes = int(spec["maximum_result_bytes"])
    if not isinstance(frame, str) or not frame or spec["alpha"] != 2 or spec["factor_count"] != 1 or not 4 <= base_resolution <= 32 or not 0 <= refinement_levels <= 4 or not all(math.isfinite(value) for value in (maximum_relative_area_error, kappa, tau, noise_sd)) or not 0.0 < maximum_relative_area_error <= 0.25 or min(kappa, tau, noise_sd) <= 0.0 or not 10 <= maximum_iterations <= 10000 or not 16 <= maximum_vertices <= 1024 or not 16 <= maximum_triangles <= 4096 or not 1 <= maximum_boundary_checks <= 100000000 or maximum_projection_visits <= 0 or not 8 <= int(spec["memory_budget_mib"]) <= 4096 or not 1024 <= maximum_result_bytes <= 16 * 1024 * 1024:
        raise ContractError("adaptive arbitrary-window SPDE controls violate their bounds")
    observations = spec["factor_observations"]
    coordinates = np.asarray([row["coordinates"] for row in observations], dtype=float)
    values = np.asarray([row["values"] for row in observations], dtype=float)
    identities = [row["region_id"] for row in observations]
    if coordinates.ndim != 2 or coordinates.shape[1] != 2 or not 8 <= len(coordinates) <= 512 or values.ndim != 2 or values.shape[0] != len(coordinates) or not 2 <= values.shape[1] <= 16 or not np.isfinite(coordinates).all() or not np.isfinite(values).all() or len(set(identities)) != len(identities) or any(not isinstance(identity, str) or not identity for identity in identities) or any(not window_contains(window, point) for point in coordinates):
        raise ContractError("adaptive SPDE requires unique finite in-window multivariate observations")
    sensitivity = []
    meshes = []
    for level in range(refinement_levels + 1):
        mesh = build_adaptive_window_mesh(window, base_resolution, level, coordinates, kappa, tau, maximum_vertices, maximum_triangles, maximum_boundary_checks, memory_budget_bytes)
        relative_area_error = abs(mesh["mesh_area"] - window["exact_area"]) / window["exact_area"]
        sensitivity.append({"boundary_refinement_level": level, "boundary_spacing": mesh["boundary_spacing"], "vertex_count": len(mesh["vertices"]), "triangle_count": len(mesh["triangles"]), "mesh_area": mesh["mesh_area"], "relative_area_error": relative_area_error})
        meshes.append(mesh)
    mesh = meshes[-1]
    relative_area_error = sensitivity[-1]["relative_area_error"]
    if relative_area_error > maximum_relative_area_error:
        raise ContractError(f"adaptive SPDE mesh relative area error {relative_area_error} exceeds tolerance {maximum_relative_area_error}")
    factor = fit_adaptive_spatial_factor(mesh, observations, noise_sd, maximum_iterations, maximum_projection_visits)
    eigenvalues = np.linalg.eigvalsh(mesh["precision"])
    projection_error = float(np.max(np.abs(factor["projection"].sum(axis=1) - 1.0)))
    result = {
        "format": "marklab.adaptive_window_spde",
        "version": 1,
        "window": {"frame": frame, "exact_area": window["exact_area"], "bounds": window["bounds"], "component_count": window["component_count"], "hole_count": window["hole_count"], "ring_count": window["ring_count"], "vertex_count": window["vertex_count"], "geometry_policy": "exact_polygon_membership_with_reported_piecewise_linear_mesh_area_error"},
        "mesh": {"basis": "piecewise_linear_triangular", "boundary_condition": "natural_neumann_with_positive_kappa", "alpha": 2, "kappa": kappa, "tau": tau, "base_resolution": base_resolution, "boundary_refinement_levels": refinement_levels, "vertices": mesh["vertices"].tolist(), "triangles": mesh["triangles"].tolist(), "vertex_count": len(mesh["vertices"]), "triangle_count": len(mesh["triangles"]), "connected_components": mesh["connected_components"], "mesh_area": mesh["mesh_area"], "relative_area_error": relative_area_error, "constant_mass_integral": float(mesh["mass"].sum()), "mass_matrix": sparse_triplets(mesh["mass"]), "stiffness_matrix": sparse_triplets(mesh["stiffness"]), "precision_matrix": sparse_triplets(mesh["precision"]), "minimum_precision_eigenvalue": float(eigenvalues[0]), "maximum_precision_eigenvalue": float(eigenvalues[-1]), "maximum_projection_row_sum_error": projection_error},
        "spatial_factor": {"inference": "fixed_hyperparameter_penalized_map", "factor_count": 1, "region_ids": identities, "feature_count": values.shape[1], "feature_means": factor["means"].tolist(), "mesh_field_weights": factor["field"].tolist(), "projected_region_factor": factor["projected"].tolist(), "loadings": factor["loadings"].tolist(), "reconstruction_rmse": factor["rmse"], "factor_x_correlation": factor["x_correlation"], "objective": factor["objective"], "gradient_maximum": factor["gradient_max"], "iterations": factor["iterations"], "projection": sparse_triplets(factor["projection"]), "projection_triangle_visits": factor["projection_visits"]},
        "mesh_sensitivity": sensitivity,
        "limits": {"maximum_vertices": maximum_vertices, "maximum_triangles": maximum_triangles, "maximum_boundary_segment_triangle_checks": maximum_boundary_checks, "maximum_projection_triangle_visits": maximum_projection_visits, "memory_budget_mib": int(spec["memory_budget_mib"]), "maximum_result_bytes": maximum_result_bytes, "maximum_iterations": maximum_iterations},
        "assumptions": ["polygonal_window_coordinates_are_physical", "piecewise_linear_finite_elements", "fixed_kappa_tau_and_one_factor", "observation_locations_not_population_replicates"],
        "limitations": ["boundary_conforming_quality_reported_by_mesh_area_error", "fixed_spde_hyperparameters", "dense_bounded_penalized_map", "within_specimen_spatial_field_diagnostic"],
        "claim_status": "fitted_arbitrary_window_spde_diagnostic",
    }
    if len(json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True).encode()) > maximum_result_bytes:
        raise ContractError("adaptive SPDE result exceeds maximum_result_bytes")
    return result


def spde_suite(spec):
    window=spec["window"]; bounds=window["bounds"]
    resolutions=[int(value) for value in spec["mesh_resolutions"]]
    kappa=float(spec["kappa"]); tau=float(spec["tau"]); maximum_iterations=int(spec["maximum_iterations"])
    if window["holes"] != [] or not isinstance(window["frame"],str) or not (0<=bounds[0]<bounds[1] and 0<=bounds[2]<bounds[3]) or spec["alpha"] != 2 or not 3<=min(resolutions)<=max(resolutions)<=16 or resolutions != sorted(set(resolutions)) or min(kappa,tau,float(spec["lgcp_beta_prior_sd"]),float(spec["factor_noise_sd"]))<=0 or not 10<=maximum_iterations<=10000:
        raise ContractError("bounded hole-free rectangular alpha-two SPDE contract violated")
    points=np.asarray(spec["lgcp_points"],float)
    if points.ndim!=2 or points.shape[1]!=2 or not 10<=len(points)<=10000 or not np.isfinite(points).all():
        raise ContractError("finite bounded LGCP points required")
    sensitivity=[]; fitted_by_resolution={}
    for resolution in resolutions:
        candidate=build_mesh(bounds,resolution,kappa,tau)
        lgcp=fit_lgcp(candidate,points,float(spec["lgcp_beta_prior_sd"]),maximum_iterations)
        fitted_by_resolution[resolution]=(candidate,lgcp)
        sensitivity.append({"resolution":resolution,"vertex_count":len(candidate["vertices"]),"triangle_count":len(candidate["triangles"]),"integrated_intensity":lgcp["integrated"],"right_to_left_mean_intensity_ratio":lgcp["right_left_ratio"]})
    resolution=resolutions[-1]; mesh,lgcp=fitted_by_resolution[resolution]
    factor=fit_spatial_factor(mesh,spec["factor_observations"],float(spec["factor_noise_sd"]),maximum_iterations)
    eigenvalues=np.linalg.eigvalsh(mesh["precision"])
    max_projection_error=max(float(np.max(np.abs(lgcp["event_projection"].sum(axis=1)-1))),float(np.max(np.abs(lgcp["quadrature_projection"].sum(axis=1)-1))),float(np.max(np.abs(factor["projection"].sum(axis=1)-1))))
    return {
        "format":"marklab.rectangular_spde_suite","version":1,
        "mesh":{"frame":window["frame"],"bounds":bounds,"holes":[],"boundary_condition":"natural_neumann_with_positive_kappa","basis":"piecewise_linear_triangular","alpha":2,"kappa":kappa,"tau":tau,"resolution":resolution,"vertices":mesh["vertices"].tolist(),"triangles":mesh["triangles"].tolist(),"vertex_count":len(mesh["vertices"]),"triangle_count":len(mesh["triangles"]),"mass_matrix":sparse_triplets(mesh["mass"]),"stiffness_matrix":sparse_triplets(mesh["stiffness"]),"precision_matrix":sparse_triplets(mesh["precision"]),"minimum_precision_eigenvalue":float(eigenvalues[0]),"maximum_projection_row_sum_error":max_projection_error},
        "lgcp":{"inference":"fixed_hyperparameter_laplace_map","beta":lgcp["beta"],"latent_field_weights":lgcp["field"].tolist(),"vertex_intensity":lgcp["vertex_intensity"].tolist(),"integrated_intensity":lgcp["integrated"],"right_to_left_mean_intensity_ratio":lgcp["right_left_ratio"],"objective":lgcp["objective"],"gradient_maximum":lgcp["gradient_max"],"iterations":lgcp["iterations"],"event_projection":sparse_triplets(lgcp["event_projection"]),"quadrature_projection":sparse_triplets(lgcp["quadrature_projection"]),"quadrature_weights":lgcp["quadrature_weights"].tolist()},
        "spatial_factor":{"inference":"fixed_hyperparameter_laplace_map","factor_count":1,"feature_means":factor["means"].tolist(),"mesh_field_weights":factor["field"].tolist(),"projected_region_factor":factor["projected"].tolist(),"loadings":factor["loadings"].tolist(),"reconstruction_rmse":factor["rmse"],"factor_x_correlation":factor["x_correlation"],"objective":factor["objective"],"iterations":factor["iterations"],"projection":sparse_triplets(factor["projection"])},
        "mesh_sensitivity":sensitivity,
        "limitations":["hole_free_rectangle_only","fixed_spde_hyperparameters","dense_bounded_laplace_map","synthetic_oracle_only"],
        "claim_status":"experimental_synthetic_rectangular_spde_lgcp_and_factor_models",
    }


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("advanced Bayesian backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if len(request_bytes) > 16 * 1024 * 1024 or request["format"] != "marklab.scipy_advanced_bayes_request" or request["version"] != 1:
        raise ContractError("request identity/size mismatch")
    if request["mode"] == "hmc_normal": result = hmc_normal(request["spec"])
    elif request["mode"] == "advanced_cluster": result = advanced_cluster(request["spec"])
    elif request["mode"] == "spde_suite": result = spde_suite(request["spec"])
    elif request["mode"] == "adaptive_window_spde": result = adaptive_window_spde(request["spec"])
    elif request["mode"] == "nonstationary_adaptive_window_spde": result = nonstationary_adaptive_window_spde(request["spec"])
    else: raise ContractError("unknown advanced Bayesian mode")
    if request["mode"] in {"adaptive_window_spde", "nonstationary_adaptive_window_spde"}:
        if abs(result["window"]["exact_area"] - float(request["canonical_window_area"])) > 1e-10 * max(1.0, float(request["canonical_window_area"])):
            raise ContractError("Rust and SciPy adaptive SPDE window areas disagree")
        result["window"]["exact_area"] = float(request["canonical_window_area"])
        result["window"]["canonical_sha256"] = request["canonical_window_sha256"]
    result["backend"] = request["backend"]
    result["request_sha256"] = hashlib.sha256(request_bytes).hexdigest()
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True); sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"advanced Bayesian worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
