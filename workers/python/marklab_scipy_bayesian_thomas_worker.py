#!/usr/bin/env python3
"""NumPy/SciPy worker for the Bayesian Thomas cluster process behind ``bayes advanced-cluster``.

The model and sampler are in ``marklab_worker_support.thomas_process``. This worker validates the
exact request contract, bounds work and kernel memory before allocation, runs the chains and
reports posterior summaries with ArviZ rank R-hat and bulk/tail ESS over all chains.

Modes: ``fit`` (observed patterns, with posterior predictive checks and an optional finite
binary Gibbs exchange fit carried over from the earlier heuristic command) and ``sbc``
(simulation-based calibration of the same sampler on patterns drawn from the prior).
"""

from __future__ import annotations

import itertools
import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)
from marklab_worker_support.thomas_process import (
    PARAMETERS,
    SEED_DOMAIN,
    box_area,
    edge_truncation_fraction,
    mean_nearest_neighbour_distance,
    parent_region,
    run_chain,
    simulate_pattern,
)

REQUEST_FORMAT = "marklab.bayesian_thomas_request"
RESULT_FORMAT = "marklab.bayesian_thomas_cluster_model"
SBC_RESULT_FORMAT = "marklab.bayesian_thomas_cluster_sbc"
BACKEND_VERSION = f"scipy-{versions.SCIPY}"
LIMITATIONS = [
    "rectangular_windows_only",
    "parents_restricted_to_declared_rectangular_dilation",
    "isotropic_gaussian_offspring_without_background_noise",
    "one_pattern_per_patient_without_covariates",
    "posterior_predictive_checks_limited_to_count_and_mean_nearest_neighbour_distance",
]
MODEL = {
    "family": "thomas_cluster_process",
    "parent_process": "homogeneous_poisson_on_rectangular_dilation",
    "offspring_kernel": "isotropic_gaussian",
    "pooling": "lognormal_hierarchy_over_patterns",
    "parent_sampler": "spatial_birth_death_move_metropolis_hastings",
}


# Request validation


def window_box(value: Any, path: str) -> tuple[float, float, float, float]:
    box = obj(value, {"xmin_um", "xmax_um", "ymin_um", "ymax_um"}, path)
    x0 = number(box["xmin_um"], f"{path}.xmin_um")
    x1 = number(box["xmax_um"], f"{path}.xmax_um")
    y0 = number(box["ymin_um"], f"{path}.ymin_um")
    y1 = number(box["ymax_um"], f"{path}.ymax_um")
    if not (x0 < x1 and y0 < y1):
        raise ContractError(f"{path} must have positive width and height")
    return x0, x1, y0, y1


def validate_priors(value: Any) -> dict[str, tuple[float, float, float]]:
    priors = obj(value, set(PARAMETERS), "model.priors")
    result = {}
    for name in PARAMETERS:
        prior = obj(
            priors[name],
            {"population_mean_mean", "population_mean_sd", "between_pattern_sd_scale"},
            f"model.priors.{name}",
        )
        mean = number(prior["population_mean_mean"], f"{name}.population_mean_mean", -50, 50)
        sd = number(prior["population_mean_sd"], f"{name}.population_mean_sd", 1e-3, 10)
        scale = number(prior["between_pattern_sd_scale"], f"{name}.between_scale", 1e-3, 10)
        result[name] = (mean, sd, scale)
    return result


def validate_model(value: Any) -> dict[str, Any]:
    model = obj(value, set(MODEL) | {"dilation_um", "priors"}, "model")
    for key, expected in MODEL.items():
        exact(model[key], expected, f"model.{key}")
    return {
        "dilation": number(model["dilation_um"], "model.dilation_um", 1e-9, 1e9),
        "priors": validate_priors(model["priors"]),
    }


def validate_sampler(value: Any) -> dict[str, int]:
    sampler = obj(
        value,
        {"chains", "warmup", "draws", "birth_death_proposals_per_sweep", "seed"},
        "sampler",
    )
    return {
        "chains": integer(sampler["chains"], "sampler.chains", 2, 16),
        "warmup": integer(sampler["warmup"], "sampler.warmup", 50, 20_000),
        "draws": integer(sampler["draws"], "sampler.draws", 50, 20_000),
        "birth_death": integer(
            sampler["birth_death_proposals_per_sweep"], "sampler.birth_death", 1, 10_000
        ),
        "seed": integer(sampler["seed"], "sampler.seed", 0, 2**64 - 1),
    }


def validate_policy(value: Any) -> dict[str, float]:
    policy = obj(
        value,
        {
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "maximum_edge_truncation_fraction",
        },
        "diagnostic_policy",
    )
    return {
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.r_hat", 1.0, 2.0),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk_ess", 1, 1e9),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail_ess", 1, 1e9),
        "maximum_edge_truncation_fraction": number(
            policy["maximum_edge_truncation_fraction"], "policy.edge_truncation", 0, 1
        ),
    }


def validate_resources(value: Any) -> dict[str, int]:
    resources = obj(
        value,
        {
            "maximum_parents_per_pattern",
            "maximum_points_per_pattern",
            "maximum_kernel_bytes",
            "maximum_work_units",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    return {
        "capacity": integer(
            resources["maximum_parents_per_pattern"], "resources.parents", 1, 100_000
        ),
        "maximum_points": integer(
            resources["maximum_points_per_pattern"], "resources.points", 2, 1_000_000
        ),
        "maximum_kernel_bytes": integer(
            resources["maximum_kernel_bytes"], "resources.kernel_bytes", 1, 8 * 1024 * MIB
        ),
        "maximum_work_units": integer(
            resources["maximum_work_units"], "resources.work_units", 1, 10**15
        ),
        "maximum_output_bytes": integer(
            resources["maximum_output_bytes"], "resources.output_bytes", 1, 16 * MIB
        ),
        "timeout_seconds": integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600),
    }


def validate_patterns(value: Any, maximum_points: int) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not 1 <= len(value) <= 64:
        raise ContractError("patterns must contain 1-64 patterns")
    patterns = []
    for index, raw in enumerate(value):
        pattern = obj(raw, {"pattern_id", "patient_id", "window", "points"}, f"patterns[{index}]")
        if not isinstance(pattern["pattern_id"], str) or not isinstance(pattern["patient_id"], str):
            raise ContractError("pattern and patient identifiers must be strings")
        window = window_box(pattern["window"], f"patterns[{index}].window")
        if (
            not isinstance(pattern["points"], list)
            or not 2 <= len(pattern["points"]) <= maximum_points
        ):
            raise ContractError(f"patterns[{index}] must contain 2-{maximum_points} points")
        points = np.asarray(
            [
                [number(point[0], "point.x"), number(point[1], "point.y")]
                if isinstance(point, list) and len(point) == 2
                else [math.nan, math.nan]
                for point in pattern["points"]
            ],
            dtype=np.float64,
        )
        if not np.isfinite(points).all():
            raise ContractError(f"patterns[{index}] points must be finite coordinate pairs")
        x0, x1, y0, y1 = window
        inside = (points[:, 0] >= x0) & (points[:, 0] <= x1)
        inside &= (points[:, 1] >= y0) & (points[:, 1] <= y1)
        if not inside.all():
            raise ContractError(f"patterns[{index}] has points outside its window")
        patterns.append(
            {
                "pattern_id": pattern["pattern_id"],
                "patient_id": pattern["patient_id"],
                "window": window,
                "points": points,
            }
        )
    if len({row["pattern_id"] for row in patterns}) != len(patterns) or len(
        {row["patient_id"] for row in patterns}
    ) != len(patterns):
        raise ContractError(
            "pattern and patient identifiers must be unique (one pattern per patient)"
        )
    return patterns


def work_units(point_counts: list[int], sampler: dict[str, int], capacity: int) -> int:
    """Upper bound on kernel evaluations: births/deaths plus displacement and scale updates."""
    sweeps = sampler["chains"] * (sampler["warmup"] + sampler["draws"])
    per_sweep = sum(n * (sampler["birth_death"] + 8 * capacity) for n in point_counts)
    return sweeps * per_sweep


def kernel_bytes(point_counts: list[int], capacity: int) -> int:
    """Live kernel storage: the current and proposed point-by-parent matrices for every pattern."""
    return 2 * 8 * capacity * sum(point_counts)


def check_budgets(point_counts, sampler, resources) -> tuple[int, int]:
    work = work_units(point_counts, sampler, resources["capacity"])
    memory = kernel_bytes(point_counts, resources["capacity"])
    if work > resources["maximum_work_units"]:
        raise ContractError("declared sampler work exceeds maximum_work_units")
    if memory > resources["maximum_kernel_bytes"]:
        raise ContractError("parent kernel storage exceeds maximum_kernel_bytes")
    return work, memory


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    if not isinstance(request, dict) or request.get("mode") not in {"fit", "sbc"}:
        raise ContractError("request.mode must be fit or sbc")
    mode = request["mode"]
    common = {"format", "version", "backend", "mode", "model", "sampler"}
    common |= {"diagnostic_policy", "resources"}
    if mode == "fit":
        keys = common | {"patterns", "posterior_predictive_draws", "finite_gibbs"}
    else:
        keys = common | {"sbc"}
    obj(request, keys, "request")
    exact(request["format"], REQUEST_FORMAT, "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="numpy_scipy_arviz",
        version=BACKEND_VERSION,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    config = {
        "mode": mode,
        "model": validate_model(request["model"]),
        "sampler": validate_sampler(request["sampler"]),
        "policy": validate_policy(request["diagnostic_policy"]),
        "resources": validate_resources(request["resources"]),
    }
    resources = config["resources"]
    if mode == "fit":
        config["patterns"] = validate_patterns(request["patterns"], resources["maximum_points"])
        config["predictive_draws"] = integer(
            request["posterior_predictive_draws"], "posterior_predictive_draws", 10, 2_000
        )
        if config["predictive_draws"] > config["sampler"]["chains"] * config["sampler"]["draws"]:
            raise ContractError("posterior_predictive_draws exceeds the retained draws")
        config["finite_gibbs"] = (
            None
            if request["finite_gibbs"] is None
            else validate_finite_gibbs(request["finite_gibbs"])
        )
        counts = [len(pattern["points"]) for pattern in config["patterns"]]
        config["work_units"], config["kernel_bytes"] = check_budgets(
            counts, config["sampler"], resources
        )
    else:
        config["sbc"] = validate_sbc(request["sbc"], resources["maximum_points"])
        sbc = config["sbc"]
        counts = [sbc["maximum_points"]] * sbc["patterns_per_replicate"]
        work, memory = check_budgets(counts, config["sampler"], resources)
        if work * sbc["replicates"] > resources["maximum_work_units"]:
            raise ContractError("declared SBC work exceeds maximum_work_units")
        config["work_units"], config["kernel_bytes"] = work * sbc["replicates"], memory
    return config


def validate_sbc(value: Any, maximum_points: int) -> dict[str, Any]:
    sbc = obj(
        value,
        {
            "window",
            "patterns_per_replicate",
            "replicates",
            "minimum_points",
            "maximum_points",
            "maximum_prior_attempts",
            "rank_draws",
            "rank_bins",
            "minimum_rank_p_value",
            "coverage_90_bounds",
        },
        "sbc",
    )
    minimum = integer(sbc["minimum_points"], "sbc.minimum_points", 2, maximum_points)
    maximum = integer(sbc["maximum_points"], "sbc.maximum_points", minimum, maximum_points)
    coverage = sbc["coverage_90_bounds"]
    if not isinstance(coverage, list) or len(coverage) != 2:
        raise ContractError("sbc.coverage_90_bounds must be [low, high]")
    low = number(coverage[0], "sbc.coverage.low", 0, 1)
    high = number(coverage[1], "sbc.coverage.high", low, 1)
    rank_draws = integer(sbc["rank_draws"], "sbc.rank_draws", 9, 999)
    bins = integer(sbc["rank_bins"], "sbc.rank_bins", 2, 50)
    if (rank_draws + 1) % bins:
        raise ContractError("sbc.rank_bins must divide rank_draws + 1")
    return {
        "window": window_box(sbc["window"], "sbc.window"),
        "patterns_per_replicate": integer(sbc["patterns_per_replicate"], "sbc.patterns", 1, 16),
        "replicates": integer(sbc["replicates"], "sbc.replicates", 1, 2_000),
        "minimum_points": minimum,
        "maximum_points": maximum,
        "maximum_prior_attempts": integer(
            sbc["maximum_prior_attempts"], "sbc.maximum_prior_attempts", 1, 1_000_000
        ),
        "rank_draws": rank_draws,
        "rank_bins": bins,
        "minimum_rank_p_value": number(sbc["minimum_rank_p_value"], "sbc.minimum_rank_p", 0, 1),
        "coverage_90_bounds": (low, high),
    }


# Diagnostics and summaries


def diagnostics_for(draws: np.ndarray) -> dict[str, float]:
    """Rank R-hat, bulk/tail ESS and mean MCSE of one quantity's ``(chain, draw)`` array."""
    import arviz as az
    import xarray as xr

    dataset = xr.Dataset({"value": (("chain", "draw"), np.asarray(draws, dtype=np.float64))})
    return {
        "r_hat": float(az.rhat(dataset, method="rank")["value"].values),
        "ess_bulk": float(az.ess(dataset, method="bulk")["value"].values),
        "ess_tail": float(az.ess(dataset, method="tail")["value"].values),
        "mcse_mean": float(az.mcse(dataset, method="mean")["value"].values),
    }


def finite_or_none(value: float) -> float | None:
    return value if math.isfinite(value) else None


def moments(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "quantile_025": float(np.quantile(flat, 0.025)),
        "median": float(np.quantile(flat, 0.5)),
        "quantile_975": float(np.quantile(flat, 0.975)),
    }


def quantity_summary(draws: np.ndarray, transform=None) -> dict[str, float | None]:
    """Posterior summary of a ``(chain, draw)`` array; ``transform`` maps it to reported units.

    Diagnostics that ArviZ cannot compute (for example a chain that never moved) are ``None``.
    """
    values = draws if transform is None else transform(draws)
    summary = moments(values)
    summary.update({key: finite_or_none(value) for key, value in diagnostics_for(values).items()})
    return summary


def stack_chains(chains, key):
    return np.stack([chain[key] for chain in chains], axis=0)


def convergence(summaries, policy):
    """Worst diagnostics over the monitored quantities; ``None`` when any is not computable."""
    keys = ("r_hat", "ess_bulk", "ess_tail", "mcse_mean")
    finite = all(summary[key] is not None for summary in summaries for key in keys)
    if not finite:
        return {
            "maximum_r_hat": None,
            "minimum_ess_bulk": None,
            "minimum_ess_tail": None,
            "all_finite": False,
            "passes": False,
        }
    worst_r_hat = max(summary["r_hat"] for summary in summaries)
    bulk = min(summary["ess_bulk"] for summary in summaries)
    tail = min(summary["ess_tail"] for summary in summaries)
    return {
        "maximum_r_hat": worst_r_hat,
        "minimum_ess_bulk": bulk,
        "minimum_ess_tail": tail,
        "all_finite": True,
        "passes": worst_r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"],
    }


def run_chains(patterns, config):
    return [
        run_chain(
            patterns,
            config["model"],
            config["sampler"],
            config["resources"]["capacity"],
            chain,
        )
        for chain in range(config["sampler"]["chains"])
    ]


# Posterior predictive checks


def posterior_predictive(patterns, theta, config):
    """Count and mean nearest-neighbour distance checks from evenly spaced posterior draws."""
    flat = theta.reshape(-1, theta.shape[2], 3)
    picks = np.linspace(0, len(flat) - 1, config["predictive_draws"]).round().astype(int)
    rng = np.random.default_rng(derive_seed(SEED_DOMAIN, config["sampler"]["seed"], "ppc"))
    rows = []
    for index, pattern in enumerate(patterns):
        observed_count = len(pattern["points"])
        observed_nn = mean_nearest_neighbour_distance(pattern["points"])
        limit = 20 * observed_count + 1000
        counts, distances, overflow = [], [], 0
        for pick in picks:
            kappa, mu, sigma = np.exp(flat[pick, index])
            points = simulate_pattern(
                rng, kappa, mu, sigma, pattern["window"], config["model"]["dilation"], limit
            )
            if points is None:
                overflow += 1
                continue
            counts.append(len(points))
            if len(points) >= 2:
                distances.append(mean_nearest_neighbour_distance(points))
        counts = np.asarray(counts, dtype=np.float64)
        distances = np.asarray(distances, dtype=np.float64)
        total = len(picks)
        rows.append(
            {
                "pattern_id": pattern["pattern_id"],
                "replicates": total,
                "overflow_replicates": overflow,
                "observed_count": observed_count,
                "count_upper_tail_probability": float(
                    (np.sum(counts >= observed_count) + overflow) / total
                ),
                "observed_mean_nearest_neighbour_um": observed_nn,
                "nearest_neighbour_replicates": int(len(distances)),
                "nearest_neighbour_upper_tail_probability": float(
                    np.mean(distances >= observed_nn) if len(distances) else 0.0
                ),
            }
        )
    return rows


# Finite binary Gibbs exchange fit (unchanged from the earlier heuristic command)


def validate_finite_gibbs(value: Any) -> dict[str, Any]:
    spec = obj(
        value,
        {
            "sites",
            "edges",
            "observed_types",
            "interaction_prior_sd",
            "proposal_sd",
            "draws",
            "burnin",
            "seed",
        },
        "finite_gibbs",
    )
    sites = integer(spec["sites"], "finite_gibbs.sites", 2, 16)
    edges = []
    for edge in spec["edges"] if isinstance(spec["edges"], list) else [None]:
        if not isinstance(edge, list) or len(edge) != 2:
            raise ContractError("finite_gibbs.edges must be index pairs")
        first = integer(edge[0], "finite_gibbs.edge", 0, sites - 1)
        second = integer(edge[1], "finite_gibbs.edge", 0, sites - 1)
        if first >= second:
            raise ContractError("finite_gibbs edges must be ordered pairs a < b")
        edges.append((first, second))
    observed = spec["observed_types"]
    if not isinstance(observed, list) or len(observed) != sites:
        raise ContractError("finite_gibbs.observed_types must have one type per site")
    observed = tuple(integer(item, "finite_gibbs.type", 0, 1) for item in observed)
    if set(observed) != {0, 1}:
        raise ContractError("finite_gibbs.observed_types must contain both types")
    return {
        "sites": sites,
        "edges": edges,
        "observed": observed,
        "prior_sd": number(spec["interaction_prior_sd"], "finite_gibbs.prior_sd", 1e-6, 1e6),
        "proposal_sd": number(spec["proposal_sd"], "finite_gibbs.proposal_sd", 1e-6, 1e6),
        "draws": integer(spec["draws"], "finite_gibbs.draws", 100, 100_000),
        "burnin": integer(spec["burnin"], "finite_gibbs.burnin", 0, 100_000),
        "seed": integer(spec["seed"], "finite_gibbs.seed", 0, 2**64 - 1),
    }


def exact_exchange(spec):
    """Exchange-algorithm posterior for the interaction of a finite binary Gibbs model.

    Model: ``P(x | theta) ∝ exp(theta * S(x))`` where ``S`` counts edges joining sites of the
    same type, with prior ``theta ~ Normal(0, interaction_prior_sd)``. Proposals are Gaussian
    random-walk steps; the auxiliary draw is exact by enumerating all ``2**sites`` states.
    """
    rng = np.random.default_rng(spec["seed"])
    edges = spec["edges"]
    observed = spec["observed"]
    states = list(itertools.product([0, 1], repeat=spec["sites"]))
    statistics = np.asarray(
        [sum(state[a] == state[b] for a, b in edges) for state in states], float
    )
    observed_stat = float(sum(observed[a] == observed[b] for a, b in edges))
    prior_sd = spec["prior_sd"]
    draws = spec["draws"]
    burnin = spec["burnin"]
    theta = 0.0
    retained = []
    accepted = 0
    for iteration in range(draws + burnin):
        proposal = theta + rng.normal(0.0, spec["proposal_sd"])
        energies = proposal * statistics
        weights = np.exp(energies - np.max(energies))
        weights /= weights.sum()
        auxiliary_stat = statistics[rng.choice(len(states), p=weights)]
        prior_term = -0.5 * (proposal**2 - theta**2) / prior_sd**2
        observed_term = (proposal - theta) * observed_stat
        auxiliary_term = (theta - proposal) * auxiliary_stat
        log_alpha = prior_term + observed_term + auxiliary_term
        if math.log(rng.uniform()) < min(0.0, log_alpha):
            theta = proposal
            if iteration >= burnin:
                accepted += 1
        if iteration >= burnin:
            retained.append(theta)
    values = np.asarray(retained)
    return {
        "posterior_interaction_mean": float(values.mean()),
        "posterior_interaction_interval_95": np.quantile(values, [0.025, 0.975]).tolist(),
        "acceptance_rate": accepted / draws,
        "observed_same_type_edge_statistic": observed_stat,
        "state_space_size": len(states),
        "auxiliary_draw_status": "exact_finite_state_enumeration",
        "relationship_to_cluster_model": "independent_of_the_thomas_cluster_model",
    }


# Fit mode


def fit(config: dict[str, Any]) -> dict[str, Any]:
    patterns = config["patterns"]
    chains = run_chains(patterns, config)
    theta = stack_chains(chains, "theta")
    parents = stack_chains(chains, "parents")
    eta = stack_chains(chains, "eta")
    tau = stack_chains(chains, "tau")
    dilation = config["model"]["dilation"]
    policy = config["policy"]
    pattern_rows = []
    monitored = []
    worst_truncation = 0.0
    for index, pattern in enumerate(patterns):
        kappa = quantity_summary(theta[:, :, index, 0], np.exp)
        mu = quantity_summary(theta[:, :, index, 1], np.exp)
        sigma = quantity_summary(theta[:, :, index, 2], np.exp)
        intensity = quantity_summary(theta[:, :, index, 0] + theta[:, :, index, 1], np.exp)
        parent_count = moments(parents[:, :, index])
        truncation = edge_truncation_fraction(
            np.exp(theta[:, :, index, 2]).reshape(-1), pattern["window"], dilation
        )
        worst_truncation = max(worst_truncation, float(truncation.max()))
        monitored.extend([kappa, mu, sigma])
        window = pattern["window"]
        pattern_rows.append(
            {
                "pattern_id": pattern["pattern_id"],
                "patient_id": pattern["patient_id"],
                "point_count": len(pattern["points"]),
                "window_area_um2": box_area(window),
                "parent_region_area_um2": box_area(parent_region(window, dilation)),
                "kappa_parent_per_um2": kappa,
                "mu_offspring": mu,
                "sigma_um": sigma,
                "offspring_intensity_per_um2": intensity,
                "parent_count_in_region": parent_count,
                "maximum_edge_truncation_fraction": float(truncation.max()),
            }
        )
    population = {}
    for parameter, name in enumerate(PARAMETERS):
        median = quantity_summary(eta[:, :, parameter], np.exp)
        spread = quantity_summary(tau[:, :, parameter])
        monitored.extend([median, spread])
        population[name] = {"population_median": median, "between_pattern_sd": spread}
    state = convergence(monitored, policy)
    capacity_hits = sum(chain["capacity_hits"] for chain in chains)
    truncation_ok = worst_truncation <= policy["maximum_edge_truncation_fraction"]
    complete = state["passes"] and capacity_hits == 0 and truncation_ok
    reasons = []
    if not state["all_finite"]:
        reasons.append("nonfinite_diagnostic")
    else:
        if state["maximum_r_hat"] > policy["maximum_r_hat"]:
            reasons.append("r_hat_above_policy")
        if state["minimum_ess_bulk"] < policy["minimum_bulk_ess"]:
            reasons.append("bulk_ess_below_policy")
        if state["minimum_ess_tail"] < policy["minimum_tail_ess"]:
            reasons.append("tail_ess_below_policy")
    if capacity_hits:
        reasons.append("parent_capacity_reached")
    if not truncation_ok:
        reasons.append("edge_truncation_above_policy")
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "fit_state": "complete" if complete else "nonconverged",
        "nonconvergence_reasons": reasons,
        "model": {**MODEL, "dilation_um": dilation},
        "patterns": pattern_rows,
        "population": population,
        "diagnostics": {
            "chains": config["sampler"]["chains"],
            "draws_per_chain": config["sampler"]["draws"],
            "warmup_per_chain": config["sampler"]["warmup"],
            "maximum_r_hat": state["maximum_r_hat"],
            "minimum_ess_bulk": state["minimum_ess_bulk"],
            "minimum_ess_tail": state["minimum_ess_tail"],
            "parent_capacity_hits": capacity_hits,
            "maximum_edge_truncation_fraction": worst_truncation,
            "monitored_quantities": len(monitored),
            "mean_acceptance": {
                key: float(np.mean([chain["acceptance"][key] for chain in chains]))
                for key in ("parameter", "shift", "spread")
            },
        },
        "posterior_predictive": posterior_predictive(patterns, theta, config),
        "resources": {
            "work_units": config["work_units"],
            "kernel_bytes": config["kernel_bytes"],
            "maximum_parents_per_pattern": config["resources"]["capacity"],
        },
        "limitations": LIMITATIONS,
        "claim_status": (
            "research_only_bayesian_thomas_cluster_posterior"
            if complete
            else "diagnostic_only_nonconverged"
        ),
    }
    if config["finite_gibbs"] is not None:
        result["finite_binary_gibbs_exchange"] = exact_exchange(config["finite_gibbs"])
    return result


# SBC mode


def prior_draw(rng, priors, count):
    eta = np.asarray([rng.normal(mean, sd) for mean, sd, _ in priors])
    tau = np.asarray([abs(rng.normal(0.0, scale)) for _, _, scale in priors])
    theta = eta[None, :] + tau[None, :] * rng.normal(size=(count, 3))
    return eta, tau, theta


def sbc(config: dict[str, Any]) -> dict[str, Any]:
    from marklab_worker_support.sbc import calibration_passes, rank_summary

    spec = config["sbc"]
    model = config["model"]
    priors = [model["priors"][name] for name in PARAMETERS]
    names = [f"population_mean_{name}" for name in PARAMETERS]
    names += [f"between_pattern_sd_{name}" for name in PARAMETERS]
    names += [f"first_pattern_{name}" for name in PARAMETERS]
    ranks = {name: [] for name in names}
    covered = {name: [] for name in names}
    replicate_rows = []
    rejected = 0
    for replicate in range(spec["replicates"]):
        rng = np.random.default_rng(
            derive_seed(SEED_DOMAIN, config["sampler"]["seed"], "sbc", replicate)
        )
        for _ in range(spec["maximum_prior_attempts"]):
            eta, tau, theta = prior_draw(rng, priors, spec["patterns_per_replicate"])
            patterns = []
            for index, values in enumerate(theta):
                kappa, mu, sigma = np.exp(values)
                points = simulate_pattern(
                    rng, kappa, mu, sigma, spec["window"], model["dilation"], spec["maximum_points"]
                )
                if points is None or len(points) < spec["minimum_points"]:
                    break
                patterns.append(
                    {
                        "pattern_id": f"r{replicate}p{index}",
                        "patient_id": f"r{replicate}p{index}",
                        "window": spec["window"],
                        "points": points,
                    }
                )
            if len(patterns) == spec["patterns_per_replicate"]:
                break
            rejected += 1
        else:
            raise ContractError("SBC prior draws repeatedly violated the declared point bounds")
        seed = derive_seed(SEED_DOMAIN, config["sampler"]["seed"], "sbc-fit", replicate, width=8)
        sampler = {**config["sampler"], "seed": seed}
        chains = run_chains(patterns, {**config, "sampler": sampler})
        eta_draws = stack_chains(chains, "eta")
        tau_draws = stack_chains(chains, "tau")
        theta_draws = stack_chains(chains, "theta")
        quantities = {}
        for parameter, name in enumerate(PARAMETERS):
            quantities[f"population_mean_{name}"] = (eta_draws[:, :, parameter], eta[parameter])
            quantities[f"between_pattern_sd_{name}"] = (tau_draws[:, :, parameter], tau[parameter])
            quantities[f"first_pattern_{name}"] = (
                theta_draws[:, :, 0, parameter],
                theta[0, parameter],
            )
        summaries = [quantity_summary(draws) for draws, _ in quantities.values()]
        state = convergence(summaries, config["policy"])
        for name, (draws, truth) in quantities.items():
            flat = draws.reshape(-1)
            picks = np.linspace(0, len(flat) - 1, spec["rank_draws"]).round().astype(int)
            thinned = flat[picks]
            ranks[name].append(int(np.sum(thinned < truth)))
            covered[name].append(bool(np.quantile(flat, 0.05) <= truth <= np.quantile(flat, 0.95)))
        replicate_rows.append(
            {
                "replicate": replicate,
                "point_counts": [len(pattern["points"]) for pattern in patterns],
                "maximum_r_hat": state["maximum_r_hat"],
                "minimum_ess_bulk": state["minimum_ess_bulk"],
                "converged": state["passes"],
                "parent_capacity_hits": sum(chain["capacity_hits"] for chain in chains),
            }
        )
    rows = []
    for name in names:
        summary = rank_summary(ranks[name], covered[name], spec["rank_draws"], spec["rank_bins"])
        rows.append({"quantity": name, **summary})
    low, high = spec["coverage_90_bounds"]
    capacity_hits = sum(row["parent_capacity_hits"] for row in replicate_rows)
    passes = calibration_passes(rows, [], spec["minimum_rank_p_value"], low, high)
    passes = passes and capacity_hits == 0
    return {
        "format": SBC_RESULT_FORMAT,
        "version": 1,
        "calibration_state": "passed" if passes else "failed",
        "claim_status": (
            "synthetic_sbc_diagnostic_not_biological_calibration"
            if passes
            else "diagnostic_only_failed_calibration"
        ),
        "nonconverged_replicates": sum(not row["converged"] for row in replicate_rows),
        "model": {**MODEL, "dilation_um": model["dilation"]},
        "replicates": spec["replicates"],
        "rejected_prior_draws": rejected,
        "rejection_rule": "redraw_when_any_simulated_pattern_violates_declared_point_bounds",
        "rank_draws": spec["rank_draws"],
        "rank_bins": spec["rank_bins"],
        "quantities": rows,
        "replicate_diagnostics": replicate_rows,
        "resources": {"work_units": config["work_units"], "kernel_bytes": config["kernel_bytes"]},
    }


def main() -> None:
    import arviz

    require_versions(numpy=np.__version__, scipy=scipy.__version__, arviz=arviz.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(64 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config) if config["mode"] == "fit" else sbc(config)
    result["backend"] = backend_identity("numpy_scipy_arviz", BACKEND_VERSION, lock_sha, worker_sha)
    result["request_sha256"] = sha256_hex(raw)
    write_result(result, max_bytes=config["resources"]["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab Bayesian Thomas worker failed", errors=(ContractError,))
