#!/usr/bin/env python3

import contextlib
import hashlib
import io
import json
import sys

import numpy as np
import sbi
import scipy
import torch
from sbi.inference import NLE, NPE, NRE
from sbi.utils import BoxUniform
from scipy.stats import truncnorm


class ContractError(Exception):
    pass


class NoOpTracker:
    @property
    def log_dir(self):
        return None

    def log_metric(self, name, value, step=None):
        pass

    def log_metrics(self, metrics, step=None):
        pass

    def log_params(self, params):
        pass

    def add_figure(self, name, figure, step=None):
        pass

    def flush(self):
        pass


def main():
    if (
        sbi.__version__ != "0.26.1"
        or torch.__version__ != "2.13.0"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("neural SBI backend version drift")
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    torch.use_deterministic_algorithms(True)
    request_bytes = sys.stdin.buffer.read(4 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.sbi_neural_estimators_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    prior_spec = request["prior"]
    simulator_spec = request["simulator"]
    prior = BoxUniform(
        low=torch.tensor([prior_spec["lower"]], dtype=torch.float32),
        high=torch.tensor([prior_spec["upper"]], dtype=torch.float32),
    )
    observed = request["observed_summary"]
    observed_tensor = torch.tensor([[observed]], dtype=torch.float32)
    grid_numpy = np.linspace(
        prior_spec["lower"], prior_spec["upper"], request["posterior_grid_points"]
    )
    grid = torch.tensor(grid_numpy[:, None], dtype=torch.float32)
    observed_grid = torch.full_like(grid, observed)
    noise_sd = simulator_spec["noise_standard_deviation"]
    a = (prior_spec["lower"] - observed) / noise_sd
    b = (prior_spec["upper"] - observed) / noise_sd
    analytic_mean = float(truncnorm.mean(a, b, loc=observed, scale=noise_sd))
    analytic_sd = float(truncnorm.std(a, b, loc=observed, scale=noise_sd))
    training = request["training"]
    captured = io.StringIO()

    def simulate(count, seed, proposal=None):
        torch.manual_seed(seed)
        theta = prior.sample((count,)) if proposal is None else proposal.sample((count,), show_progress_bars=False)
        summary = theta + noise_sd * torch.randn_like(theta)
        return theta, summary

    def density_summary(log_density):
        values = np.asarray(log_density.detach().cpu(), dtype=float).reshape(-1)
        density = np.exp(values - values.max())
        normalization = float(np.trapezoid(density, grid_numpy))
        density /= normalization
        mean = float(np.trapezoid(density * grid_numpy, grid_numpy))
        variance = float(np.trapezoid(density * (grid_numpy - mean) ** 2, grid_numpy))
        return mean, variance**0.5

    def serialize_state(estimator):
        buffer = io.BytesIO()
        torch.save(estimator.state_dict(), buffer)
        state = buffer.getvalue()
        return len(state), hashlib.sha256(state).hexdigest()

    theta, summary = simulate(request["simulations_per_estimator"], request["seed"])
    estimators = {}
    method_specs = [
        ("npe", NPE, {"density_estimator": training["density_estimator"]}),
        ("nle", NLE, {"density_estimator": training["density_estimator"]}),
        ("nre", NRE, {"classifier": training["ratio_classifier"]}),
    ]
    with contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
        for method_index, (method, inference_type, arguments) in enumerate(method_specs):
            torch.manual_seed(request["seed"] + method_index + 1)
            inference = inference_type(
                prior=prior,
                device="cpu",
                tracker=NoOpTracker(),
                show_progress_bars=False,
                **arguments,
            )
            estimator = inference.append_simulations(theta, summary).train(
                training_batch_size=training["batch_size"],
                max_num_epochs=training["maximum_epochs"],
                stop_after_epochs=training["stop_after_epochs"],
                show_train_summary=False,
            )
            with torch.no_grad():
                if method == "npe":
                    log_density = estimator.log_prob(grid, observed_grid)
                elif method == "nle":
                    log_density = estimator.log_prob(observed_grid, grid)
                else:
                    log_density = estimator.unnormalized_log_ratio(grid, observed_grid)
            posterior_mean, posterior_sd = density_summary(log_density)
            state_bytes, state_sha = serialize_state(estimator)
            estimators[method] = {
                "estimand": {
                    "npe": "posterior_density_theta_given_x",
                    "nle": "likelihood_density_x_given_theta",
                    "nre": "likelihood_to_evidence_ratio",
                }[method],
                "architecture": arguments,
                "simulation_count": request["simulations_per_estimator"],
                "posterior_mean": posterior_mean,
                "posterior_standard_deviation": posterior_sd,
                "posterior_mean_absolute_error": abs(posterior_mean - analytic_mean),
                "posterior_sd_absolute_error": abs(posterior_sd - analytic_sd),
                "serialized_state_bytes": state_bytes,
                "serialized_state_sha256": state_sha,
            }

        sequential_inference = NPE(
            prior=prior,
            density_estimator=training["density_estimator"],
            device="cpu",
            tracker=NoOpTracker(),
            show_progress_bars=False,
        )
        proposal = None
        sequential_estimator = None
        round_records = []
        for round_index in range(request["sequential_rounds"]):
            theta_round, summary_round = simulate(
                request["simulations_per_round"],
                request["seed"] + 100 + round_index,
                proposal,
            )
            bank_bytes = theta_round.detach().numpy().tobytes() + summary_round.detach().numpy().tobytes()
            sequential_estimator = sequential_inference.append_simulations(
                theta_round,
                summary_round,
                proposal=proposal,
            ).train(
                training_batch_size=training["batch_size"],
                max_num_epochs=training["maximum_epochs"],
                stop_after_epochs=training["stop_after_epochs"],
                show_train_summary=False,
                retrain_from_scratch=False,
            )
            proposal = sequential_inference.build_posterior(sequential_estimator).set_default_x(
                observed_tensor
            )
            round_records.append({
                "round": round_index + 1,
                "proposal": "prior" if round_index == 0 else f"round_{round_index}_npe_posterior",
                "simulation_count": len(theta_round),
                "theta_minimum": float(theta_round.min()),
                "theta_maximum": float(theta_round.max()),
                "simulation_bank_sha256": hashlib.sha256(bank_bytes).hexdigest(),
            })
    with torch.no_grad():
        sequential_log_density = sequential_estimator.log_prob(grid, observed_grid)
    sequential_mean, sequential_sd = density_summary(sequential_log_density)
    sequential_state_bytes, sequential_state_sha = serialize_state(sequential_estimator)
    if len({record["simulation_bank_sha256"] for record in round_records}) != len(round_records):
        raise ContractError("sequential simulation banks are not isolated")
    result = {
        "format": "marklab.neural_simulation_based_inference",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "prior": prior_spec,
        "simulator": simulator_spec,
        "observed_summary": observed,
        "analytic_oracle": {
            "family": "truncated_gaussian_posterior",
            "mean": analytic_mean,
            "standard_deviation": analytic_sd,
        },
        "estimators": estimators,
        "sequential": {
            "method": "npe",
            "rounds": round_records,
            "posterior_mean": sequential_mean,
            "posterior_standard_deviation": sequential_sd,
            "posterior_mean_absolute_error": abs(sequential_mean - analytic_mean),
            "posterior_sd_absolute_error": abs(sequential_sd - analytic_sd),
            "serialized_state_bytes": sequential_state_bytes,
            "serialized_state_sha256": sequential_state_sha,
        },
        "diagnostics": {
            "posterior_grid_points": request["posterior_grid_points"],
            "captured_backend_log_lines": len(captured.getvalue().splitlines()),
            "device": "cpu",
            "deterministic_algorithms": True,
        },
        "claim_status": "experimental_synthetic_amortized_and_sequential_sbi",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, RuntimeError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"neural SBI worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
