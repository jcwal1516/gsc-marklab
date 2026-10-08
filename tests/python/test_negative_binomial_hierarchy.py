import importlib.util
import copy
import hashlib
import json
from pathlib import Path
import unittest

import numpy as np
from numpyro.infer.util import log_density
from scipy.stats import halfnorm, nbinom, norm


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_numpyro_negative_binomial_hierarchy_worker.py"
)


def load_worker():
    specification = importlib.util.spec_from_file_location(
        "negative_binomial_hierarchy_worker", MODULE_PATH
    )
    module = importlib.util.module_from_spec(specification)
    assert specification.loader is not None
    specification.loader.exec_module(module)
    return module


def fixture_spec():
    """Frozen synthetic 16-patient caller shared with the real CLI/replay test."""
    path = Path(__file__).resolve().parents[1] / "fixtures/negative_binomial_hierarchy.json"
    return json.loads(path.read_text())


def validation_request(worker, spec, lock_digest, worker_digest):
    spec = copy.deepcopy(spec)
    spec["observations"].sort(key=lambda r: (r["patient_id"], r["slide_id"], r["roi_id"]))
    request = {
        "format": "marklab.numpyro_negative_binomial_hierarchy_request",
        "version": 1,
        "backend": {
            "name": "numpyro", "version": "0.21.0", "python_version": "3.12",
            "environment_lock_sha256": lock_digest, "worker_sha256": worker_digest,
        },
        "jax_version": "0.11.1", "spec": spec,
        "design": worker.compile_design(spec["observations"]),
        "diagnostic_policy": {
            "prior_predictive_draws": 500, "maximum_r_hat": 1.01,
            "minimum_bulk_ess": 400.0, "minimum_tail_ess": 400.0, "minimum_ebfmi": 0.3,
            "maximum_divergences": 0, "maximum_tree_depth_hits": 0, "maximum_tree_depth": 10,
        },
    }
    raw = json.dumps(request, sort_keys=True, separators=(",", ":")).encode()
    return worker.validate(request, lock_digest, worker_digest), hashlib.sha256(raw).hexdigest()


def independent_pymc_fit(spec):
    """Independent model construction and likelihood implementation for one agreement oracle."""
    import pymc as pm

    rows = spec["observations"]
    patients, patient_index = np.unique([r["patient_id"] for r in rows], return_inverse=True)
    slides, slide_index = np.unique([r["slide_id"] for r in rows], return_inverse=True)
    predictor = np.asarray([r["predictor"] for r in rows])
    centered = np.zeros_like(predictor)
    for patient in range(len(patients)):
        selected = patient_index == patient
        centered[selected] = predictor[selected] - predictor[selected].mean()
    within = centered / np.sqrt(np.mean(centered**2))
    areas = np.asarray([r["area"] for r in rows])
    counts = np.asarray([r["count"] for r in rows])
    p = spec["priors"]
    with pm.Model():
        alpha = pm.Normal("alpha", p["intercept_mean"], p["intercept_sd"])
        beta = pm.Normal("beta", 0.0, p["slope_sd"])
        log_dispersion = pm.Normal("log_dispersion", p["log_dispersion_mean"], p["log_dispersion_sd"])
        s0 = pm.HalfNormal("patient_intercept_sd", p["patient_intercept_sd_scale"])
        s1 = pm.HalfNormal("patient_slope_sd", p["patient_slope_sd_scale"])
        ss = pm.HalfNormal("slide_sd", p["slide_sd_scale"])
        z0 = pm.Normal("patient_intercept_z", 0.0, 1.0, shape=len(patients))
        z1 = pm.Normal("patient_slope_z", 0.0, 1.0, shape=len(patients))
        zs = pm.Normal("slide_z", 0.0, 1.0, shape=len(slides))
        eta = alpha + beta * within + s0*z0[patient_index] + s1*z1[patient_index]*within + ss*zs[slide_index]
        pm.NegativeBinomial("count", mu=areas*pm.math.exp(eta), alpha=pm.math.exp(log_dispersion), observed=counts)
        sampling = spec["sampling"]
        fitted = pm.sample(
            chains=sampling["chains"], cores=1, tune=sampling["tune_per_chain"],
            draws=sampling["draws_per_chain"], target_accept=sampling["target_accept"],
            nuts={"max_treedepth": 10}, random_seed=20260914, progressbar=False,
            compute_convergence_checks=False,
        )
    return fitted


def run_scientific_validation(output_path, *, include_agreement=True, sbc_replicates=20):
    """Opt-in, prespecified validation of this workflow; ordinary tests stay focused/fast."""
    import arviz as az
    from scipy.stats import binomtest

    worker = load_worker()
    lock_digest = hashlib.sha256(MODULE_PATH.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(MODULE_PATH.read_bytes()).hexdigest()
    spec = fixture_spec()
    config, request_digest = validation_request(worker, spec, lock_digest, worker_digest)
    sampler = worker.build_sampler(config)
    result, samples = worker.fit(config, request_digest, lock_digest, worker_digest, sampler=sampler, return_samples=True)
    samples["dispersion"] = np.exp(samples["log_dispersion"])
    quantities = ["alpha", "beta", "dispersion", "patient_intercept_sd", "patient_slope_sd", "slide_sd"]
    truth = dict(alpha=-1.0, beta=0.7, dispersion=12.0, patient_intercept_sd=0.35, patient_slope_sd=0.25, slide_sd=0.2)
    report = {
        "scope": "synthetic_implementation_calibration_smoke_not_real_study_calibration",
        "baseline": {"fit_state": result["fit_state"], "diagnostics": result["diagnostics"],
            "posterior": result["posterior"], "truth": truth,
            "truth_in_interval": {k: result["posterior"][k]["interval_lower"] <= truth[k] <= result["posterior"][k]["interval_upper"] for k in quantities},
            "prior_predictive": result["prior_predictive"], "posterior_predictive": result["posterior_predictive"]},
    }
    path = Path(output_path)
    def save():
        path.write_text(json.dumps(report, indent=2, allow_nan=False)+"\n")
    save()
    print("baseline", result["fit_state"], result["diagnostics"], flush=True)
    if result["fit_state"] != "complete":
        raise AssertionError("baseline diagnostics failed; evidence retained")

    if include_agreement:
        fitted = independent_pymc_fit(spec)
        independent = {name: np.asarray(fitted["posterior"][name].values) for name in samples if name in fitted["posterior"]}
        independent["dispersion"] = np.exp(independent["log_dispersion"])
        diagnostic_names = ["alpha", "beta", "log_dispersion", "patient_intercept_sd", "patient_slope_sd", "slide_sd",
            "patient_intercept_z", "patient_slope_z", "slide_z"]
        def extrema(tree, operation):
            return float(operation(np.concatenate([np.asarray(tree[k]).reshape(-1) for k in diagnostic_names])))
        energy = np.asarray(fitted["sample_stats"]["energy"])
        independent_diagnostics = {
            "r_hat": extrema(az.rhat(fitted, var_names=diagnostic_names, method="rank"), np.max),
            "ess_bulk": extrema(az.ess(fitted, var_names=diagnostic_names, method="bulk"), np.min),
            "ess_tail": extrema(az.ess(fitted, var_names=diagnostic_names, method="tail"), np.min),
            "ebfmi": float(np.min(np.mean(np.diff(energy,axis=1)**2,axis=1)/np.var(energy,axis=1))),
            "divergences": int(np.asarray(fitted["sample_stats"]["diverging"]).sum()),
            "depth_hits": int(np.asarray(fitted["sample_stats"]["reached_max_treedepth"]).sum()),
        }
        numpyro_data = az.from_dict({"posterior": {k: samples[k] for k in quantities}})
        pymc_data = az.from_dict({"posterior": {k: independent[k] for k in quantities}})
        nmcse = az.mcse(numpyro_data, var_names=quantities, method="mean")
        pmcse = az.mcse(pymc_data, var_names=quantities, method="mean")
        nsmcse = az.mcse(numpyro_data, var_names=quantities, method="sd")
        psmcse = az.mcse(pymc_data, var_names=quantities, method="sd")
        comparison = {}
        for k in quantities:
            nm, pm = float(samples[k].mean()), float(independent[k].mean())
            ns, ps = float(samples[k].std(ddof=1)), float(independent[k].std(ddof=1))
            pooled = np.sqrt((ns**2+ps**2)/2)
            mean_limit = 4*np.hypot(float(nmcse[k]),float(pmcse[k]))+0.05*pooled
            sd_limit = 0.1*pooled+4*np.hypot(float(nsmcse[k]),float(psmcse[k]))
            comparison[k] = {"numpyro_mean": nm,"pymc_mean": pm,"mean_difference": abs(nm-pm),"mean_tolerance": float(mean_limit),
                "numpyro_sd": ns,"pymc_sd": ps,"sd_difference": abs(ns-ps),"sd_tolerance": float(sd_limit),
                "passed": bool(abs(nm-pm)<=mean_limit and abs(ns-ps)<=sd_limit)}
        report["agreement"] = {"pymc_diagnostics": independent_diagnostics,"parameters":comparison}
        save()
        print("agreement", independent_diagnostics, comparison, flush=True)
        d=independent_diagnostics
        assert d["r_hat"]<=1.01 and min(d["ess_bulk"],d["ess_tail"])>=400 and d["ebfmi"]>=.3 and d["divergences"]==d["depth_hits"]==0, "independent diagnostics failed; evidence retained"
        assert all(x["passed"] for x in comparison.values()), "backend agreement failed; evidence retained"

    sensitivity=[]
    for family, multiplier in [("hierarchy",0.5),("hierarchy",2.0),("slope",0.5),("slope",2.0)]:
        alternative=copy.deepcopy(spec)
        keys=["patient_intercept_sd_scale","patient_slope_sd_scale","slide_sd_scale"] if family=="hierarchy" else ["slope_sd"]
        for key in keys:
            alternative["priors"][key]*=multiplier
        altered_config,digest=validation_request(worker,alternative,lock_digest,worker_digest)
        altered=worker.fit(altered_config,digest,lock_digest,worker_digest,sampler=sampler)
        shifts={k:(altered["posterior"][k]["mean"]-result["posterior"][k]["mean"])/result["posterior"][k]["sd"] for k in quantities}
        sensitivity.append({"family":family,"multiplier":multiplier,"fit_state":altered["fit_state"],"diagnostics":altered["diagnostics"],"posterior":altered["posterior"],"baseline_sd_shifts":shifts})
        report["sensitivity"]=sensitivity
        save()
        print("sensitivity",family,multiplier,altered["fit_state"],shifts,flush=True)

    rng=np.random.default_rng(20260915)
    rows=spec["observations"]
    patient_ids,patient_index=np.unique([r["patient_id"] for r in rows],return_inverse=True)
    slide_ids,slide_index=np.unique([r["slide_id"] for r in rows],return_inverse=True)
    raw_x=np.asarray([r["predictor"] for r in rows])
    centered=np.empty_like(raw_x)
    for p in range(len(patient_ids)):
        chosen=patient_index==p
        centered[chosen]=raw_x[chosen]-raw_x[chosen].mean()
    within=centered/np.sqrt(np.mean(centered**2))
    area=np.asarray([r["area"] for r in rows])
    priors=spec["priors"]
    sbc=[]
    ranks={k:[] for k in quantities}
    covered={k:0 for k in quantities}
    selected_draws=63
    for replicate in range(sbc_replicates):
        actual={"alpha":float(rng.normal(priors["intercept_mean"],priors["intercept_sd"])),
            "beta":float(rng.normal(0,priors["slope_sd"])),
            "dispersion":float(np.exp(rng.normal(priors["log_dispersion_mean"],priors["log_dispersion_sd"]))),
            "patient_intercept_sd":float(abs(rng.normal(0,priors["patient_intercept_sd_scale"]))),
            "patient_slope_sd":float(abs(rng.normal(0,priors["patient_slope_sd_scale"]))),
            "slide_sd":float(abs(rng.normal(0,priors["slide_sd_scale"])))}
        b0=rng.normal(0,actual["patient_intercept_sd"],len(patient_ids))
        b1=rng.normal(0,actual["patient_slope_sd"],len(patient_ids))
        bs=rng.normal(0,actual["slide_sd"],len(slide_ids))
        mu=area*np.exp(actual["alpha"]+actual["beta"]*within+b0[patient_index]+b1[patient_index]*within+bs[slide_index])
        simulated=copy.deepcopy(spec)
        y=rng.negative_binomial(actual["dispersion"],actual["dispersion"]/(actual["dispersion"]+mu))
        for row,count in zip(simulated["observations"],y):
            row["count"]=int(count)
        simulated["sampling"]["seed"]=20261000+replicate
        simulated_config,digest=validation_request(worker,simulated,lock_digest,worker_digest)
        try:
            fitted,posterior_samples=worker.fit(simulated_config,digest,lock_digest,worker_digest,sampler=sampler,return_samples=True)
            posterior_samples["dispersion"] = np.exp(posterior_samples["log_dispersion"])
            disposition={"replicate":replicate,"truth":actual,"fit_state":fitted["fit_state"],"diagnostics":fitted["diagnostics"]}
            if fitted["fit_state"]=="complete":
                disposition["ranks"]={}
                disposition["covered"]={}
                for k in quantities:
                    flat=posterior_samples[k].reshape(-1)
                    selected=flat[np.linspace(0,len(flat)-1,selected_draws,dtype=int)]
                    less=int(np.count_nonzero(selected<actual[k]))
                    tied=int(np.count_nonzero(selected==actual[k]))
                    rank=less+int(rng.integers(tied+1))
                    ranks[k].append(rank)
                    lo,hi=np.quantile(flat,[.025,.975])
                    hit=bool(lo<=actual[k]<=hi)
                    covered[k]+=int(hit)
                    disposition["ranks"][k]=rank
                    disposition["covered"][k]=hit
        except Exception as error:
            disposition={"replicate":replicate,"truth":actual,"fit_state":"error","error":f"{type(error).__name__}: {error}"}
        sbc.append(disposition)
        report["sbc"]={"replicates":sbc,"selected_posterior_draws":selected_draws,"ranks":ranks}
        save()
        print("sbc",replicate,disposition["fit_state"],disposition.get("diagnostics",disposition.get("error")),flush=True)

    complete=sum(x["fit_state"]=="complete" for x in sbc)
    support=np.arange(selected_draws+1)
    expected_masses=np.bincount((support*5)//(selected_draws+1),minlength=5)/(selected_draws+1)
    calibration={}
    for k in quantities:
        if complete:
            counts=np.bincount(ranks[k],minlength=selected_draws+1)
            empirical=np.cumsum(counts)/complete
            discrete_null=(support+1)/(selected_draws+1)
            interval=binomtest(covered[k],complete).proportion_ci()
            calibration[k]={"covered":covered[k],"evaluated":complete,"coverage":covered[k]/complete,
                "coverage_exact_95_interval":[float(interval.low),float(interval.high)],
                "pointwise_nominal_coverage_compatible":bool(interval.low<=.95<=interval.high),
                "rank_bin_counts":np.bincount(np.asarray(ranks[k])*5//(selected_draws+1),minlength=5).tolist(),
                "expected_bin_counts":(complete*expected_masses).tolist(),
                "discrete_ecdf_max_error":float(np.max(np.abs(empirical-discrete_null))),
                "dkw_95_bound":float(np.sqrt(np.log(40)/(2*complete)))}
    report["sbc"].update({"completed":complete,"attempted":sbc_replicates,"quantities":calibration,
        "status":("complete_smoke_only" if all(x["pointwise_nominal_coverage_compatible"] for x in calibration.values())
            else "complete_smoke_with_coverage_warnings") if complete==sbc_replicates else "incomplete",
        "rank_assumption":"63 spaced NUTS draws; independence is approximate; rank checks are smoke evidence",
        "coverage_denominator":"completed fits only; any missing fit prevents calibration completion"})
    save()
    print("calibration summary",report["sbc"]["status"],calibration,flush=True)
    return report


class NegativeBinomialHierarchyMathTest(unittest.TestCase):
    def test_model_nb2_likelihood_and_complete_joint_density_match_scipy(self):
        worker = load_worker()
        counts = np.asarray([0, 3, 2, 8, 1, 5], dtype=np.int64)
        areas = np.asarray([0.5, 1.2, 0.8, 2.0, 1.5, 0.7])
        within = np.asarray([-1.1, 0.4, 0.7, -0.8, 0.1, 0.7])
        patient_index = np.asarray([0, 0, 0, 1, 1, 1], dtype=np.int32)
        slide_index = np.asarray([0, 0, 1, 2, 2, 3], dtype=np.int32)
        priors = {
            "intercept_mean": -0.2,
            "intercept_sd": 1.3,
            "slope_sd": 0.9,
            "log_dispersion_mean": 1.0,
            "log_dispersion_sd": 0.6,
            "patient_intercept_sd_scale": 0.7,
            "patient_slope_sd_scale": 0.5,
            "slide_sd_scale": 0.4,
        }
        state = {
            "alpha": 0.15,
            "beta": 0.55,
            "log_dispersion": np.log(4.2),
            "patient_intercept_sd": 0.35,
            "patient_slope_sd": 0.22,
            "slide_sd": 0.18,
            "patient_intercept_z": np.asarray([-0.4, 0.8]),
            "patient_slope_z": np.asarray([0.3, -0.6]),
            "slide_z": np.asarray([-0.5, 0.2, 0.7, -0.1]),
        }

        eta = (
            state["alpha"]
            + state["beta"] * within
            + state["patient_intercept_sd"]
            * state["patient_intercept_z"][patient_index]
            + state["patient_slope_sd"]
            * state["patient_slope_z"][patient_index]
            * within
            + state["slide_sd"] * state["slide_z"][slide_index]
        )
        means = areas * np.exp(eta)
        dispersion = np.exp(state["log_dispersion"])
        probability = dispersion / (dispersion + means)
        expected = float(np.sum(nbinom.logpmf(counts, dispersion, probability)))
        expected += float(norm.logpdf(state["alpha"], priors["intercept_mean"], priors["intercept_sd"]))
        expected += float(norm.logpdf(state["beta"], 0.0, priors["slope_sd"]))
        expected += float(
            norm.logpdf(
                state["log_dispersion"],
                priors["log_dispersion_mean"],
                priors["log_dispersion_sd"],
            )
        )
        expected += float(
            halfnorm.logpdf(
                state["patient_intercept_sd"],
                scale=priors["patient_intercept_sd_scale"],
            )
        )
        expected += float(
            halfnorm.logpdf(
                state["patient_slope_sd"],
                scale=priors["patient_slope_sd_scale"],
            )
        )
        expected += float(
            halfnorm.logpdf(state["slide_sd"], scale=priors["slide_sd_scale"])
        )
        expected += float(np.sum(norm.logpdf(state["patient_intercept_z"], 0.0, 1.0)))
        expected += float(np.sum(norm.logpdf(state["patient_slope_z"], 0.0, 1.0)))
        expected += float(np.sum(norm.logpdf(state["slide_z"], 0.0, 1.0)))

        model_actual, trace = log_density(
            worker.model,
            (),
            {
                "counts": worker.jnp.asarray(counts),
                "areas": worker.jnp.asarray(areas),
                "within_predictor": worker.jnp.asarray(within),
                "patient_index": worker.jnp.asarray(patient_index),
                "slide_index": worker.jnp.asarray(slide_index),
                "patient_template": worker.jnp.zeros(2),
                "slide_template": worker.jnp.zeros(4),
                "priors": priors,
            },
            state,
        )
        likelihood_site = trace["count_log_likelihood"]
        likelihood_actual = float(
            np.asarray(
                likelihood_site["fn"].log_prob(likelihood_site["value"])
            ).sum()
        )
        likelihood_expected = float(
            np.sum(nbinom.logpmf(counts, dispersion, probability))
        )
        self.assertAlmostEqual(likelihood_actual, likelihood_expected, delta=1e-10)
        self.assertAlmostEqual(float(model_actual), expected, delta=1e-8)

    def test_predictor_compilation_is_stable_to_large_coordinate_origin(self):
        worker = load_worker()

        def observations(origin):
            rows = []
            for patient in range(8):
                for slide in range(2):
                    for roi, predictor in enumerate((0.0, 0.0, 0.0, 1.0)):
                        rows.append(
                            {
                                "patient_id": f"patient-{patient:02d}",
                                "slide_id": f"slide-{patient:02d}-{slide}",
                                "roi_id": f"roi-{patient:02d}-{slide}-{roi}",
                                "count": 0,
                                "area": 1.0,
                                "predictor": origin + predictor,
                            }
                        )
            return rows

        reference = worker.compile_design(observations(0.0))
        shifted = worker.compile_design(observations(float(2**52)))

        np.testing.assert_array_equal(
            shifted["within_predictor"], reference["within_predictor"]
        )
        self.assertEqual(shifted["within_predictor_sd"], reference["within_predictor_sd"])


if __name__ == "__main__":
    unittest.main()
