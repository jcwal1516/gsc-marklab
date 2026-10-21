"""Bayesian Thomas cluster process: truncated likelihood, parent state and MCMC chain.

Model, for each pattern ``j`` with rectangular window ``W_j`` and parent region ``B_j``, the
rectangle ``W_j`` widened by the declared dilation ``R`` on every side:

* parents ``C_j`` form a homogeneous Poisson process of intensity ``kappa_j`` on ``B_j``;
* given ``C_j`` the observed points form a Poisson process on ``W_j`` with intensity
  ``lambda_j(u) = mu_j * sum_{c in C_j} N2(u; c, sigma_j^2 I)`` (Gaussian offspring);
* ``(log kappa_j, log mu_j, log sigma_j)`` are independent ``Normal(eta_p, tau_p^2)`` across
  patterns, with ``eta_p ~ Normal(m_p, s_p^2)`` and ``tau_p ~ HalfNormal(h_p)``.

Parents beyond ``B_j`` are excluded by the model; ``edge_truncation_fraction`` is the exact share
of the untruncated process's offspring intensity in ``W_j`` that such parents would contribute.

Sampling (Geyer and Moller 1994; Moller and Waagepetersen 2004): spatial
birth-death proposals (births uniform on ``B_j`` or near a data point; uniform deaths), with or
without an intensity-preserving rescaling of ``mu_j``; Gaussian parent displacements;
random-walk Metropolis for the log parameters; Gibbs updates for ``eta_p``; and for each ``p``
a centred and a non-centred update of the hierarchy (a common shift of ``eta_p`` and every
``theta_jp``; a rescaling of ``tau_p`` and the deviations ``theta_jp - eta_p``). Step sizes adapt
only during warmup.
"""

from __future__ import annotations

import math

import numpy as np
from scipy.spatial import cKDTree
from scipy.special import ndtr

from .protocol import ContractError, derive_seed

SEED_DOMAIN = "marklab-bayesian-thomas-v1"
PARAMETERS = ("log_kappa", "log_mu", "log_sigma")
TWO_PI = 2.0 * math.pi
TARGET_ACCEPTANCE = 0.44
DISPLACEMENT_TARGET_ACCEPTANCE = 0.3
CANCELLATION_RATIO = 1e-9
UNIFORM_BIRTH_SHARE = 0.5
LOG_LIMIT = 300.0
"""Log parameters and ``log tau`` outside ``(-LOG_LIMIT, LOG_LIMIT)`` have zero target density."""


# Geometry and the truncated Thomas likelihood


def parent_region(window, dilation):
    x0, x1, y0, y1 = window
    return (x0 - dilation, x1 + dilation, y0 - dilation, y1 + dilation)


def box_area(box) -> float:
    return (box[1] - box[0]) * (box[3] - box[2])


def normal_loss(t: np.ndarray) -> np.ndarray:
    """``phi(t) - t * Phi(-t)``: the expected excess ``E[(Z - t)+]`` of a standard normal."""
    density = np.exp(-0.5 * t * t) / math.sqrt(TWO_PI)
    return np.maximum(density - t * ndtr(-t), 0.0)


def edge_truncation_fraction(sigma: np.ndarray, window, dilation: float) -> np.ndarray:
    """Share of the untruncated process's offspring intensity in ``W`` from parents beyond ``B``.

    For one axis of width ``w``, ``int_{B} P(c + sigma Z in W) dc = w - 2 sigma (L(R/sigma) -
    L((w + R)/sigma))`` with ``L`` the normal loss function, so the missing fraction is
    ``1 - (1 - dx / w)(1 - dy / h)`` with ``dx = 2 sigma (L(R/sigma) - L((w + R)/sigma))``.
    """
    sigma = np.asarray(sigma, dtype=np.float64)
    width = window[1] - window[0]
    height = window[3] - window[2]

    def deficit(length):
        return (
            2.0 * sigma * (normal_loss(dilation / sigma) - normal_loss((length + dilation) / sigma))
        )

    retained = (1.0 - deficit(width) / width) * (1.0 - deficit(height) / height)
    return np.clip(1.0 - retained, 0.0, 1.0)


def window_probability(parents: np.ndarray, sigma: float, window) -> np.ndarray:
    """``P(c + sigma Z in W)`` for each parent ``c``: a product of normal CDF differences."""
    x0, x1, y0, y1 = window
    px = ndtr((x1 - parents[:, 0]) / sigma) - ndtr((x0 - parents[:, 0]) / sigma)
    py = ndtr((y1 - parents[:, 1]) / sigma) - ndtr((y0 - parents[:, 1]) / sigma)
    return px * py


def single_window_probability(parent: np.ndarray, sigma: float, window) -> float:
    """``window_probability`` for one parent, using scalar ``erfc`` to avoid array overhead."""
    x0, x1, y0, y1 = window
    scale = sigma * math.sqrt(2.0)
    px = 0.5 * (math.erfc((parent[0] - x1) / scale) - math.erfc((parent[0] - x0) / scale))
    py = 0.5 * (math.erfc((parent[1] - y1) / scale) - math.erfc((parent[1] - y0) / scale))
    return px * py


def kernel_column(points: np.ndarray, parent: np.ndarray, sigma: float) -> np.ndarray:
    """``N2(x; parent, sigma^2 I)`` for every point."""
    dx = points[:, 0] - parent[0]
    dy = points[:, 1] - parent[1]
    return np.exp((dx * dx + dy * dy) * (-0.5 / (sigma * sigma))) / (TWO_PI * sigma * sigma)


def gaussian_kernel(points: np.ndarray, parents: np.ndarray, sigma: float) -> np.ndarray:
    """``N2(x; c, sigma^2 I)`` for every point (rows) and parent (columns)."""
    dx = points[:, 0, None] - parents[None, :, 0]
    dy = points[:, 1, None] - parents[None, :, 1]
    return np.exp(-0.5 * (dx * dx + dy * dy) / (sigma * sigma)) / (TWO_PI * sigma * sigma)


def log_likelihood(points, parents, mu, sigma, window) -> float:
    """Direct Thomas log likelihood given parents (oracle for the incremental state)."""
    density = gaussian_kernel(points, parents, sigma).sum(axis=1)
    if len(points) and np.any(density <= 0.0):
        return -math.inf
    expected = mu * float(window_probability(parents, sigma, window).sum())
    return len(points) * math.log(mu) + float(np.log(density).sum()) - expected


class PatternState:
    """Parents and cached kernel sums of one pattern in one chain."""

    def __init__(self, points, window, dilation, capacity, theta, rng):
        self.points = points
        self.count = len(points)
        self.window = window
        self.region = parent_region(window, dilation)
        self.region_area = box_area(self.region)
        self.capacity = capacity
        self.theta = np.asarray(theta, dtype=np.float64)
        self.parents = np.zeros((capacity, 2))
        self.kernel = np.zeros((self.count, capacity))
        self.scratch = np.zeros((self.count, capacity))
        self.probability = np.zeros(capacity)
        self.size = 0
        self.capacity_hits = 0
        self.displacement_scale = 0.5
        self.initial_covering(rng)
        self.refresh()

    @property
    def kappa(self) -> float:
        return math.exp(self.theta[0])

    @property
    def mu(self) -> float:
        return math.exp(self.theta[1])

    @property
    def sigma(self) -> float:
        return math.exp(self.theta[2])

    def initial_covering(self, rng):
        """Place parents on data points until every point lies within a covering radius.

        The radius starts at ``3 sigma`` and doubles while the covering would use more than half
        the parent capacity. Up to ``24 sigma`` every point keeps a positive kernel value.
        """
        order = rng.permutation(self.count)
        radius = 3.0 * self.sigma
        while radius <= 24.0 * self.sigma:
            chosen = []
            uncovered = np.ones(self.count, dtype=bool)
            for index in order:
                if uncovered[index]:
                    chosen.append(index)
                    distance = np.sum((self.points - self.points[index]) ** 2, axis=1)
                    uncovered &= distance > radius * radius
            if 2 * len(chosen) <= self.capacity:
                self.size = len(chosen)
                self.parents[: self.size] = self.points[chosen]
                return
            radius *= 2.0
        raise ContractError("initial parent covering exceeds maximum_parents_per_pattern")

    def refresh(self):
        """Recompute the kernel matrix, point densities and window probabilities exactly."""
        parents = self.parents[: self.size]
        self.kernel[:, : self.size] = gaussian_kernel(self.points, parents, self.sigma)
        self.density = self.kernel[:, : self.size].sum(axis=1)
        self.probability[: self.size] = window_probability(parents, self.sigma, self.window)
        self.probability_sum = float(self.probability[: self.size].sum())

    def point_log_density(self) -> float:
        if np.any(self.density <= 0.0):
            return -math.inf
        return float(np.log(self.density).sum())

    def log_ratio(self, proposed) -> float:
        if np.any(proposed <= 0.0):
            return -math.inf
        return float(np.log(proposed / self.density).sum())

    def without(self, index, column=None):
        """Point densities with parent ``index`` removed (and ``column`` added), guarding loss."""
        proposed = self.density - self.kernel[:, index]
        lost = proposed <= CANCELLATION_RATIO * self.density
        if lost.any():
            others = np.r_[0:index, index + 1 : self.size]
            proposed[lost] = self.kernel[np.ix_(lost, others)].sum(axis=1)
        if column is not None:
            proposed = proposed + column
        return proposed

    def uniform_parent(self, rng) -> np.ndarray:
        x0, x1, y0, y1 = self.region
        return np.asarray([rng.uniform(x0, x1), rng.uniform(y0, y1)])

    def birth_density(self, column_sum: float) -> float:
        """Density of the birth proposal at a location whose kernel column sums to ``column_sum``.

        Births are uniform on the parent region with probability ``UNIFORM_BIRTH_SHARE`` and
        otherwise a uniformly chosen point plus ``Normal(0, sigma^2 I)``; the second component's
        density at ``c`` is ``sum_i N2(c; x_i, sigma^2 I) / n``, the kernel column sum over ``n``.
        Locations outside the region are proposed with this density and always rejected.
        """
        uniform = UNIFORM_BIRTH_SHARE / self.region_area
        return uniform + (1.0 - UNIFORM_BIRTH_SHARE) * column_sum / self.count

    def propose_birth_location(self, rng) -> np.ndarray:
        if rng.uniform() < UNIFORM_BIRTH_SHARE:
            return self.uniform_parent(rng)
        anchor = self.points[int(rng.integers(self.count))]
        return anchor + rng.normal(0.0, self.sigma, 2)

    def inside_region(self, location) -> bool:
        x0, x1, y0, y1 = self.region
        return x0 <= location[0] <= x1 and y0 <= location[1] <= y1

    def birth_or_death(self, rng, mu_mean: float, mu_sd: float):
        """One spatial birth-death proposal (equal birth and death probabilities).

        With probability one half the proposal keeps ``mu``; otherwise it also rescales ``mu`` so
        that the expected window count ``mu * sum_c P(c + sigma Z in W)`` is unchanged. That
        rescaling is a translation of ``log mu`` determined by the parents, so it has unit
        Jacobian and is reversed exactly by the matching death or birth.
        """
        preserve = rng.uniform() < 0.5
        log_kappa = float(self.theta[0])
        log_mu = float(self.theta[1])
        if rng.uniform() < 0.5:
            if self.size == self.capacity:
                self.capacity_hits += 1
                return
            parent = self.propose_birth_location(rng)
            if not self.inside_region(parent):
                return
            column = kernel_column(self.points, parent, self.sigma)
            probability = single_window_probability(parent, self.sigma, self.window)
            proposed = self.density + column
            new_sum = self.probability_sum + probability
            new_log_mu = log_mu
            if preserve:
                if self.probability_sum <= 0.0:
                    return
                new_log_mu = log_mu + math.log(self.probability_sum) - math.log(new_sum)
                if not -LOG_LIMIT < new_log_mu < LOG_LIMIT:
                    return
            log_r = self.log_ratio(proposed)
            log_r += self.count * (new_log_mu - log_mu)
            log_r -= math.exp(new_log_mu) * new_sum - math.exp(log_mu) * self.probability_sum
            log_r += log_normal_density(new_log_mu, mu_mean, mu_sd)
            log_r -= log_normal_density(log_mu, mu_mean, mu_sd)
            log_r += log_kappa - math.log(self.size + 1)
            log_r -= math.log(self.birth_density(float(column.sum())))
            if math.log(rng.uniform()) < log_r:
                self.parents[self.size] = parent
                self.kernel[:, self.size] = column
                self.probability[self.size] = probability
                self.size += 1
                self.density = proposed
                self.probability_sum = new_sum
                self.theta[1] = new_log_mu
            return
        if self.size == 0:
            return
        index = int(rng.integers(self.size))
        proposed = self.without(index)
        new_sum = self.probability_sum - self.probability[index]
        new_log_mu = log_mu
        if preserve:
            if new_sum <= 0.0:
                return
            new_log_mu = log_mu + math.log(self.probability_sum) - math.log(new_sum)
            if not -LOG_LIMIT < new_log_mu < LOG_LIMIT:
                return
        log_r = self.log_ratio(proposed)
        log_r += self.count * (new_log_mu - log_mu)
        log_r -= math.exp(new_log_mu) * new_sum - math.exp(log_mu) * self.probability_sum
        log_r += log_normal_density(new_log_mu, mu_mean, mu_sd)
        log_r -= log_normal_density(log_mu, mu_mean, mu_sd)
        log_r += math.log(self.size) - log_kappa
        log_r += math.log(self.birth_density(float(self.kernel[:, index].sum())))
        if math.log(rng.uniform()) < log_r:
            last = self.size - 1
            self.parents[index] = self.parents[last]
            self.kernel[:, index] = self.kernel[:, last]
            self.probability[index] = self.probability[last]
            self.size = last
            self.density = proposed
            self.probability_sum = new_sum
            self.theta[1] = new_log_mu

    def displace_all(self, rng) -> float:
        """Propose a Gaussian displacement of every parent in turn; return the acceptance rate."""
        if self.size == 0:
            return 0.0
        step = self.displacement_scale * self.sigma
        x0, x1, y0, y1 = self.region
        accepted = 0
        for index in range(self.size):
            parent = self.parents[index] + rng.normal(0.0, step, 2)
            if not (x0 <= parent[0] <= x1 and y0 <= parent[1] <= y1):
                continue
            column = kernel_column(self.points, parent, self.sigma)
            probability = single_window_probability(parent, self.sigma, self.window)
            proposed = self.without(index, column)
            log_r = self.log_ratio(proposed) - self.mu * (probability - self.probability[index])
            if math.log(rng.uniform()) < log_r:
                self.parents[index] = parent
                self.kernel[:, index] = column
                self.probability_sum += probability - self.probability[index]
                self.probability[index] = probability
                self.density = proposed
                accepted += 1
        return accepted / self.size

    def log_likelihood_terms(self, parameter: int, value: float) -> float:
        """Log likelihood terms that depend on ``theta[parameter]``, evaluated at ``value``.

        ``parameter`` 0 is ``log kappa`` (parent prior), 1 is ``log mu`` and 2 is ``log sigma``.
        The ``log sigma`` evaluation leaves the proposed kernel in ``self.scratch``.
        """
        if not -LOG_LIMIT < value < LOG_LIMIT:
            return -math.inf
        if parameter == 0:
            return self.size * value - math.exp(value) * self.region_area
        if parameter == 1:
            return self.count * value - math.exp(value) * self.probability_sum
        sigma = math.exp(value)
        parents = self.parents[: self.size]
        self.scratch[:, : self.size] = gaussian_kernel(self.points, parents, sigma)
        density = self.scratch[:, : self.size].sum(axis=1)
        if self.count and np.any(density <= 0.0):
            return -math.inf
        self.proposed_density = density
        self.proposed_probability = window_probability(parents, sigma, self.window)
        expected = self.mu * float(self.proposed_probability.sum())
        return float(np.log(density).sum()) - expected

    def update_parameter(self, rng, parameter: int, step: float, mean: float, sd: float) -> bool:
        """Random-walk Metropolis update of ``theta[parameter]`` under ``Normal(mean, sd^2)``."""
        current = float(self.theta[parameter])
        proposal = current + step * rng.normal()
        log_r = self.log_likelihood_terms(parameter, proposal) - self.current_terms(parameter)
        log_r += log_normal_density(proposal, mean, sd) - log_normal_density(current, mean, sd)
        move = math.log(rng.uniform()) < log_r
        if move:
            self.accept_value(parameter, proposal)
        return move

    def current_terms(self, parameter: int) -> float:
        if parameter == 2:
            return self.point_log_density() - self.mu * self.probability_sum
        return self.log_likelihood_terms(parameter, float(self.theta[parameter]))

    def accept_value(self, parameter: int, value: float):
        self.theta[parameter] = value
        if parameter == 2:
            self.kernel, self.scratch = self.scratch, self.kernel
            self.density = self.proposed_density
            self.probability[: self.size] = self.proposed_probability
            self.probability_sum = float(self.proposed_probability.sum())


class Adaptation:
    """Random-walk scale adapted during warmup by a Robbins-Monro rule on its logarithm."""

    def __init__(self, scale: float, target: float):
        self.log_scale = math.log(scale)
        self.target = target

    @property
    def scale(self) -> float:
        return math.exp(self.log_scale)

    def update(self, acceptance: float, iteration: int, warmup: int):
        if iteration < warmup:
            self.log_scale += (acceptance - self.target) / (iteration + 1) ** 0.6
            self.log_scale = min(max(self.log_scale, math.log(1e-4)), math.log(3.0))


def log_normal_density(value, mean, sd) -> float:
    return -math.log(sd) - 0.5 * ((value - mean) / sd) ** 2


def log_half_normal_on_log_scale(log_tau, scale) -> float:
    """Half-normal log density of ``tau`` plus the ``log tau`` Jacobian."""
    if not -LOG_LIMIT < log_tau < LOG_LIMIT:
        return -math.inf
    tau = math.exp(log_tau)
    return -0.5 * (tau / scale) ** 2 + log_tau


def metropolis(rng, log_ratio: float) -> bool:
    return math.log(rng.uniform()) < log_ratio


def run_chain(patterns, model, sampler, capacity, chain):
    """Run one chain; return the retained draws and sampler diagnostics."""
    rng = np.random.default_rng(derive_seed(SEED_DOMAIN, sampler["seed"], "chain", chain))
    priors = [model["priors"][name] for name in PARAMETERS]
    eta = np.asarray([rng.normal(mean, sd) for mean, sd, _ in priors])
    log_tau = np.asarray([math.log(0.5 * scale) for _, _, scale in priors])
    states = []
    for pattern in patterns:
        theta = eta + 0.25 * np.exp(log_tau) * rng.normal(size=3)
        states.append(
            PatternState(
                pattern["points"], pattern["window"], model["dilation"], capacity, theta, rng
            )
        )
    count = len(states)
    own = [[Adaptation(0.2, TARGET_ACCEPTANCE) for _ in range(3)] for _ in states]
    shift = [Adaptation(0.1, TARGET_ACCEPTANCE) for _ in range(3)]
    spread = [Adaptation(0.3, TARGET_ACCEPTANCE) for _ in range(3)]
    displacement = [Adaptation(0.5, DISPLACEMENT_TARGET_ACCEPTANCE) for _ in states]
    warmup = sampler["warmup"]
    total = warmup + sampler["draws"]
    theta_draws = np.zeros((sampler["draws"], count, 3))
    parent_draws = np.zeros((sampler["draws"], count))
    eta_draws = np.zeros((sampler["draws"], 3))
    tau_draws = np.zeros((sampler["draws"], 3))
    accepted = {"parameter": 0, "shift": 0, "spread": 0}
    proposals = {"parameter": 0, "shift": 0, "spread": 0}
    for iteration in range(total):
        for index, state in enumerate(states):
            state.refresh()
            for _ in range(sampler["birth_death"]):
                state.birth_or_death(rng, eta[1], math.exp(log_tau[1]))
            state.displacement_scale = displacement[index].scale
            displacement[index].update(state.displace_all(rng), iteration, warmup)
            for parameter in range(3):
                move = state.update_parameter(
                    rng,
                    parameter,
                    own[index][parameter].scale,
                    eta[parameter],
                    math.exp(log_tau[parameter]),
                )
                own[index][parameter].update(float(move), iteration, warmup)
                proposals["parameter"] += 1
                accepted["parameter"] += move
        for parameter, (prior_mean, prior_sd, scale) in enumerate(priors):
            values = np.asarray([state.theta[parameter] for state in states])
            tau = math.exp(log_tau[parameter])
            precision = 1.0 / prior_sd**2 + count / tau**2
            center = (prior_mean / prior_sd**2 + values.sum() / tau**2) / precision
            eta[parameter] = rng.normal(center, 1.0 / math.sqrt(precision))
            # Centred move: shift eta and every pattern value together.
            delta = shift[parameter].scale * rng.normal()
            log_r = sum(
                state.log_likelihood_terms(parameter, float(state.theta[parameter]) + delta)
                - state.current_terms(parameter)
                for state in states
            )
            log_r += log_normal_density(eta[parameter] + delta, prior_mean, prior_sd)
            log_r -= log_normal_density(eta[parameter], prior_mean, prior_sd)
            move = metropolis(rng, log_r)
            if move:
                eta[parameter] += delta
                for state in states:
                    state.log_likelihood_terms(parameter, float(state.theta[parameter]) + delta)
                    state.accept_value(parameter, float(state.theta[parameter]) + delta)
            shift[parameter].update(float(move), iteration, warmup)
            proposals["shift"] += 1
            accepted["shift"] += move
            # Non-centred move: rescale tau and the standardized deviations together.
            proposal = log_tau[parameter] + spread[parameter].scale * rng.normal()
            ratio = math.exp(min(proposal - log_tau[parameter], LOG_LIMIT))
            targets = [
                float(eta[parameter] + ratio * (state.theta[parameter] - eta[parameter]))
                for state in states
            ]
            log_r = sum(
                state.log_likelihood_terms(parameter, target) - state.current_terms(parameter)
                for state, target in zip(states, targets, strict=True)
            )
            log_r += log_half_normal_on_log_scale(proposal, scale)
            log_r -= log_half_normal_on_log_scale(log_tau[parameter], scale)
            move = metropolis(rng, log_r)
            if move:
                log_tau[parameter] = proposal
                for state, target in zip(states, targets, strict=True):
                    state.log_likelihood_terms(parameter, target)
                    state.accept_value(parameter, target)
            spread[parameter].update(float(move), iteration, warmup)
            proposals["spread"] += 1
            accepted["spread"] += move
            # Centred tau update given the pattern values.
            proposal = log_tau[parameter] + 0.3 * rng.normal()
            log_r = log_half_normal_on_log_scale(proposal, scale)
            log_r -= log_half_normal_on_log_scale(log_tau[parameter], scale)
            if math.isfinite(log_r):
                values = [float(state.theta[parameter]) for state in states]
                log_r += sum(
                    log_normal_density(value, eta[parameter], math.exp(proposal))
                    - log_normal_density(value, eta[parameter], math.exp(log_tau[parameter]))
                    for value in values
                )
            if metropolis(rng, log_r):
                log_tau[parameter] = proposal
        if iteration >= warmup:
            row = iteration - warmup
            theta_draws[row] = [state.theta for state in states]
            parent_draws[row] = [state.size for state in states]
            eta_draws[row] = eta
            tau_draws[row] = np.exp(log_tau)
    for state in states:
        state.refresh()
        if not np.isfinite(state.point_log_density()):
            raise ContractError("a chain ended with a point of zero offspring intensity")
    return {
        "theta": theta_draws,
        "parents": parent_draws,
        "eta": eta_draws,
        "tau": tau_draws,
        "capacity_hits": sum(state.capacity_hits for state in states),
        "acceptance": {key: accepted[key] / max(proposals[key], 1) for key in accepted},
    }


def simulate_pattern(rng, kappa, mu, sigma, window, dilation, maximum_points):
    """Simulate the truncated Thomas model; return ``None`` when it exceeds ``maximum_points``."""
    region = parent_region(window, dilation)
    parent_count = rng.poisson(kappa * box_area(region))
    if parent_count > 4 * maximum_points + 1000:
        return None
    parents = np.column_stack(
        [
            rng.uniform(region[0], region[1], parent_count),
            rng.uniform(region[2], region[3], parent_count),
        ]
    )
    offspring = rng.poisson(mu, parent_count)
    total = int(offspring.sum())
    if total > 50 * maximum_points + 10_000:
        return None
    points = np.repeat(parents, offspring, axis=0) + rng.normal(0.0, sigma, (total, 2))
    inside = (points[:, 0] >= window[0]) & (points[:, 0] <= window[1])
    inside &= (points[:, 1] >= window[2]) & (points[:, 1] <= window[3])
    points = points[inside]
    if len(points) > maximum_points:
        return None
    return points


def mean_nearest_neighbour_distance(points: np.ndarray) -> float:
    if len(points) < 2:
        return math.nan
    distances, _ = cKDTree(points).query(points, k=2)
    return float(distances[:, 1].mean())
