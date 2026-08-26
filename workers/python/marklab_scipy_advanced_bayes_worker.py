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
    else: raise ContractError("unknown advanced Bayesian mode")
    result["backend"] = request["backend"]
    result["request_sha256"] = hashlib.sha256(request_bytes).hexdigest()
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True); sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"advanced Bayesian worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
