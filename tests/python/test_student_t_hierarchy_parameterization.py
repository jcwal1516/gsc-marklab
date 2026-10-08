#!/usr/bin/env python3

import importlib.util
from pathlib import Path
import unittest
from unittest import mock

import numpy as np
import arviz as az


WORKER_DIRECTORY = Path(__file__).resolve().parents[2] / "workers" / "python"


def load_worker(name: str):
    path = WORKER_DIRECTORY / name
    specification = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(specification)
    assert specification.loader is not None
    specification.loader.exec_module(module)
    return module


class StudentTObservationSdTest(unittest.TestCase):
    def test_both_backends_preserve_declared_finite_variance_sd(self):
        observation_sd = np.asarray([0.5, 1.0, 3.0])
        degrees_of_freedom = np.asarray([3.0, 5.0, 20.0])
        expected_scale = observation_sd * np.sqrt(
            (degrees_of_freedom - 2.0) / degrees_of_freedom
        )

        for worker_name in (
            "marklab_pymc_student_t_hierarchy_worker.py",
            "marklab_numpyro_student_t_hierarchy_worker.py",
        ):
            with self.subTest(worker=worker_name):
                worker = load_worker(worker_name)
                actual_scale = np.asarray(
                    worker.student_t_scale_from_sd(
                        observation_sd, degrees_of_freedom
                    )
                )
                np.testing.assert_allclose(
                    actual_scale, expected_scale, rtol=1e-15, atol=0.0
                )
                implied_variance = (
                    actual_scale**2
                    * degrees_of_freedom
                    / (degrees_of_freedom - 2.0)
                )
                np.testing.assert_allclose(
                    implied_variance, observation_sd**2, rtol=1e-15, atol=0.0
                )

    def test_numpyro_model_likelihood_uses_converted_scale(self):
        worker = load_worker("marklab_numpyro_student_t_hierarchy_worker.py")
        observation_sd = 3.0
        degrees_of_freedom = 5.0
        expected_scale = observation_sd * np.sqrt(
            (degrees_of_freedom - 2.0) / degrees_of_freedom
        )
        conditioned = worker.numpyro.handlers.substitute(
            worker.model,
            data={
                "global_mean": 0.0,
                "between_patient_sd": 1.0,
                "patient_z": worker.jnp.asarray([0.0]),
                "observation_scale": expected_scale,
                "degrees_of_freedom_excess": degrees_of_freedom - 2.0,
            },
        )
        trace = worker.numpyro.handlers.trace(conditioned).get_trace(
            patient_index=worker.jnp.asarray([0]),
            observations=worker.jnp.asarray([0.0]),
            patient_count=1,
            global_mean=0.0,
            global_sd=1.0,
            between_sd=1.0,
            observation_sd=1.0,
            df_rate=1.0,
        )

        actual_scale = float(trace["observation"]["fn"].scale[0])
        self.assertAlmostEqual(actual_scale, expected_scale, places=15)
        np.testing.assert_allclose(
            float(trace["observation_sd"]["value"]),
            observation_sd,
            rtol=1e-15,
            atol=0.0,
        )

    def test_numpyro_scale_coordinates_keep_the_declared_observation_sd_prior(self):
        from numpyro.infer.util import potential_energy

        worker = load_worker("marklab_numpyro_student_t_hierarchy_worker.py")
        numpyro, dist, jnp = worker.numpyro, worker.dist, worker.jnp

        def declared_sd_model(
            patient_index,
            observations,
            patient_count,
            global_mean,
            global_sd,
            between_sd,
            observation_sd,
            df_rate,
        ):
            population = numpyro.sample("global_mean", dist.Normal(global_mean, global_sd))
            between = numpyro.sample("between_patient_sd", dist.HalfNormal(between_sd))
            patient_z = numpyro.sample(
                "patient_z", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1)
            )
            sd = numpyro.sample("observation_sd", dist.HalfNormal(observation_sd))
            excess = numpyro.sample("degrees_of_freedom_excess", dist.Exponential(df_rate))
            degrees = 2.0 + excess
            numpyro.sample(
                "observation",
                dist.StudentT(
                    degrees,
                    population + between * patient_z[patient_index],
                    sd * jnp.sqrt(excess / degrees),
                ),
                obs=observations,
            )

        arguments = {
            "patient_index": jnp.asarray([0, 0, 1]),
            "observations": jnp.asarray([0.2, -0.4, 1.5]),
            "patient_count": 2,
            "global_mean": 0.1,
            "global_sd": 1.0,
            "between_sd": 0.5,
            "observation_sd": 0.5,
            "df_rate": 0.2,
        }
        # Unconstrained coordinates; the map from log sd to log scale is a shift,
        # so both parameterizations must have identical potential energy.
        for log_sd, log_excess in [(-6.0, -3.0), (-1.0, 0.5), (0.7, 3.0)]:
            with self.subTest(log_sd=log_sd, log_excess=log_excess):
                shared = {
                    "global_mean": jnp.asarray(0.3),
                    "between_patient_sd": jnp.asarray(-0.7),
                    "patient_z": jnp.asarray([0.4, -1.1]),
                    "degrees_of_freedom_excess": jnp.asarray(log_excess),
                }
                excess = np.exp(log_excess)
                log_scale = log_sd + 0.5 * np.log(excess / (2.0 + excess))
                declared = potential_energy(
                    declared_sd_model, (), arguments,
                    {**shared, "observation_sd": jnp.asarray(log_sd)},
                )
                reparameterized = potential_energy(
                    worker.model, (), arguments,
                    {**shared, "observation_scale": jnp.asarray(log_scale)},
                )
                self.assertAlmostEqual(float(reparameterized), float(declared), places=10)

    def test_prior_diagnostics_check_generated_observations(self):
        pymc_worker = load_worker("marklab_pymc_student_t_hierarchy_worker.py")
        prior = az.from_dict({
            "prior": {"global_mean": np.zeros((1, 2))},
            "prior_predictive": {"observation": np.zeros((1, 2, 4))},
        })
        self.assertTrue(pymc_worker.prior_predictive_is_finite(prior))
        prior.prior_predictive["observation"].values[0, 0, 0] = np.inf
        self.assertFalse(pymc_worker.prior_predictive_is_finite(prior))

        worker = load_worker("marklab_numpyro_student_t_hierarchy_worker.py")

        class NonfiniteObservationRng:
            def normal(self, loc=0.0, scale=1.0, size=None):
                return np.ones(size)

            def exponential(self, scale=1.0, size=None):
                return np.ones(size)

            def standard_t(self, degrees_of_freedom, size=None):
                return np.full(size, np.inf)

        config = {
            "seed": 11,
            "prior_draws": 4,
            "patient_ids": ["p0", "p1"],
            "global_mean": 0.0,
            "global_sd": 1.0,
            "between_sd": 1.0,
            "observation_sd": 1.0,
            "df_rate": 1.0,
        }
        patient_index = np.asarray([0, 0, 1, 1])
        self.assertTrue(worker.prior_predictive_is_finite(config, patient_index))
        with mock.patch.object(
            worker.np.random, "default_rng", return_value=NonfiniteObservationRng()
        ):
            finite = worker.prior_predictive_is_finite(config, patient_index)

        self.assertFalse(finite)


if __name__ == "__main__":
    unittest.main()
