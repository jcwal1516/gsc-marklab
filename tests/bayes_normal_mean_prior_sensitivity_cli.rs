#![cfg(feature = "cli")]

use std::{fs, path::Path, time::Duration};

use assert_cmd::Command;
use marklab_bayes::{
    evaluate_normal_mean_prior_sensitivity, sha256_hex, NormalMeanPriorSensitivitySpec,
    NormalPriorAlternative, PriorSensitivityWorkerRequest, PriorSensitivityWorkerResult,
};

const OBSERVATIONS: &str = "observation\n1\n2\n3\n4\n";
const PRIORS: &str = "prior_name,prior_mean,prior_sd\nbase,0,1\nskeptical,-2,0.5\nwide,0,10\n";

fn native_command(observations: &Path, priors: &Path, output: &Path) -> Command {
    let directory = output.parent().unwrap();
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command
        .env("MARKLAB_RUNTIME_ROOT", directory.join("no-runtime"))
        .env("MARKLAB_PYTHON", directory.join("no-python"))
        .env("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", "1")
        .args(["bayes", "normal-mean-prior-sensitivity", "--input"])
        .arg(observations)
        .arg("--priors")
        .arg(priors)
        .args([
            "--base-prior",
            "base",
            "--known-sigma",
            "1",
            "--decision-threshold",
            "1",
            "--decision-probability-threshold",
            "0.95",
            "--material-mean-shift",
            "0.5",
            "--timeout-seconds",
            "60",
            "--out",
        ])
        .arg(output);
    command
}

#[test]
fn conjugate_prior_grid_reports_posterior_predictive_and_decision_changes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let observations = directory.path().join("observations.csv");
    let priors = directory.path().join("priors.csv");
    let output = directory.path().join("sensitivity.json");
    fs::write(&observations, OBSERVATIONS).expect("observations");
    fs::write(&priors, PRIORS).expect("priors");
    native_command(&observations, &priors, &output)
        .assert()
        .success();

    let bytes = fs::read(&output).unwrap();
    let result: serde_json::Value = serde_json::from_slice(&bytes).expect("sensitivity JSON");
    assert_eq!(
        result["format"],
        "marklab.bayesian_normal_mean_prior_sensitivity"
    );
    assert_eq!(result["version"], 2);
    assert_eq!(result["backend"], "native_rust");
    assert_eq!(result["base_prior"], "base");
    assert_eq!(result["priors"].as_array().unwrap().len(), 3);
    let base = result["priors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|prior| prior["prior_name"] == "base")
        .unwrap();
    assert!((base["posterior_mean"].as_f64().unwrap() - 2.0).abs() <= 1e-12);
    assert!((base["posterior_sd"].as_f64().unwrap() - 0.2_f64.sqrt()).abs() <= 1e-12);
    assert_eq!(base["decision"], true);
    assert!(base["loo_elpd"].as_f64().unwrap().is_finite());
    assert_eq!(
        result["conclusion_changed_priors"],
        serde_json::json!(["skeptical"])
    );
    assert_eq!(
        result["material_mean_shift_priors"],
        serde_json::json!(["skeptical"])
    );
    assert_eq!(result["sensitivity_state"], "decision_sensitive");
    assert_eq!(result["claim_status"], "experimental_sensitivity_analysis");

    let repeated_output = directory.path().join("repeated.json");
    native_command(&observations, &priors, &repeated_output)
        .assert()
        .success();
    assert_eq!(bytes, fs::read(&repeated_output).unwrap());
    native_command(&observations, &priors, &output)
        .assert()
        .failure();
    assert_eq!(bytes, fs::read(&output).unwrap());
}

#[test]
fn invalid_or_oversized_inputs_do_not_publish_results() {
    let directory = tempfile::tempdir().unwrap();
    let observations = directory.path().join("observations.csv");
    let priors = directory.path().join("priors.csv");
    let output = directory.path().join("sensitivity.json");
    for (observation_csv, prior_csv) in [
        ("value\n1\n2\n", PRIORS),
        ("observation\n1\nNaN\n", PRIORS),
        ("observation\n1\nbad\n", PRIORS),
        (
            OBSERVATIONS,
            "prior_name,prior_mean,prior_sd\nbase,0,1\nbase,0,2\n",
        ),
        ("observation\n1e200\n-1e200\n", PRIORS),
    ] {
        fs::write(&observations, observation_csv).unwrap();
        fs::write(&priors, prior_csv).unwrap();
        native_command(&observations, &priors, &output)
            .assert()
            .failure();
        assert!(!output.exists());
    }
    for oversized in [&observations, &priors] {
        fs::write(&observations, OBSERVATIONS).unwrap();
        fs::write(&priors, PRIORS).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(oversized)
            .unwrap()
            .set_len(16 * 1024 * 1024 + 1)
            .unwrap();
        native_command(&observations, &priors, &output)
            .assert()
            .failure()
            .stderr(predicates::str::contains("16 MiB limit"));
        assert!(!output.exists());
    }
}

#[test]
fn native_scientific_results_agree_with_independent_scipy_worker() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker = repository.join("workers/python/marklab_scipy_prior_sensitivity_worker.py");
    let interpreter = marklab::python_backend_interpreter(repository).unwrap();
    let lock_digest = sha256_hex(&fs::read(repository.join("workers/python/uv.lock")).unwrap());
    let worker_digest = sha256_hex(&fs::read(&worker).unwrap());
    let priors = vec![
        NormalPriorAlternative {
            prior_name: "base".into(),
            prior_mean: 0.0,
            prior_sd: 1.0,
        },
        NormalPriorAlternative {
            prior_name: "skeptical".into(),
            prior_mean: -2.0,
            prior_sd: 0.5,
        },
        NormalPriorAlternative {
            prior_name: "wide".into(),
            prior_mean: 0.0,
            prior_sd: 10.0,
        },
    ];
    for (observations, sigma, threshold) in [
        (vec![1.0, 2.0, 3.0, 4.0], 1.0, 1.0),
        (vec![-120.0, 30.0], 25.0, -20.0),
        (vec![1.0, 2.0, 3.0, 4.0], 1.0, 2.0 + 10.0 * 0.2_f64.sqrt()),
    ] {
        let specification = NormalMeanPriorSensitivitySpec {
            observations,
            priors: priors.clone(),
            base_prior: "base".into(),
            known_sigma: sigma,
            decision_threshold: threshold,
            decision_probability_threshold: 0.95,
            material_mean_shift: 0.5,
        };
        let native = evaluate_normal_mean_prior_sensitivity(specification.clone(), 60).unwrap();
        let request = PriorSensitivityWorkerRequest::new(
            specification,
            lock_digest.clone(),
            worker_digest.clone(),
            60,
        )
        .unwrap();
        let request_bytes = serde_json::to_vec(&request).unwrap();
        let request_digest = sha256_hex(&request_bytes);
        let assertion = Command::from_std(marklab::python_backend_command(&interpreter, &worker))
            .timeout(Duration::from_secs(60))
            .write_stdin(request_bytes)
            .assert()
            .success();
        let mut reference: PriorSensitivityWorkerResult =
            serde_json::from_slice(&assertion.get_output().stdout).unwrap();
        reference.validate(&request, &request_digest).unwrap();
        assert_eq!(native.observation_count, reference.observation_count);
        assert_eq!(native.observed_mean, reference.observed_mean);
        assert_eq!(
            native.conclusion_changed_priors,
            reference.conclusion_changed_priors
        );
        assert_eq!(
            native.material_mean_shift_priors,
            reference.material_mean_shift_priors
        );
        for (actual, expected) in native.priors.iter().zip(&reference.priors) {
            assert_eq!(actual.prior_name, expected.prior_name);
            assert_eq!(actual.decision, expected.decision);
            assert_eq!(actual.conclusion_changed, expected.conclusion_changed);
            assert_eq!(actual.material_mean_shift, expected.material_mean_shift);
            for (left, right) in [
                (actual.posterior_mean, expected.posterior_mean),
                (actual.posterior_sd, expected.posterior_sd),
                (actual.loo_elpd, expected.loo_elpd),
                (
                    actual.mean_difference_from_base,
                    expected.mean_difference_from_base,
                ),
                (
                    actual.sd_difference_from_base,
                    expected.sd_difference_from_base,
                ),
                (
                    actual.probability_difference_from_base,
                    expected.probability_difference_from_base,
                ),
                (
                    actual.loo_elpd_difference_from_base,
                    expected.loo_elpd_difference_from_base,
                ),
            ] {
                assert!(
                    (left - right).abs() <= 1e-10 * right.abs().max(1.0),
                    "{}: native {left}, SciPy {right}",
                    actual.prior_name
                );
            }
            // Relative tolerance must not accept a cancelled-to-zero upper tail.
            let probability = expected.probability_above_threshold;
            assert!(
                (actual.probability_above_threshold - probability).abs() <= 1e-10 * probability,
                "{}: native {}, SciPy {probability}",
                actual.prior_name,
                actual.probability_above_threshold
            );
        }
        // A common probability shift leaves all deltas unchanged; the validator must
        // independently check the probability itself, not trust a self-consistent worker.
        if threshold == 1.0 {
            let mut boundary_request = request.clone();
            boundary_request.model.decision_probability_threshold = (native.priors[0]
                .probability_above_threshold
                + reference.priors[0].probability_above_threshold)
                / 2.0;
            let boundary_bytes = serde_json::to_vec(&boundary_request).unwrap();
            let boundary_digest = sha256_hex(&boundary_bytes);
            let boundary_output =
                Command::from_std(marklab::python_backend_command(&interpreter, &worker))
                    .timeout(Duration::from_secs(60))
                    .write_stdin(boundary_bytes)
                    .assert()
                    .success();
            let boundary_reference: PriorSensitivityWorkerResult =
                serde_json::from_slice(&boundary_output.get_output().stdout).unwrap();
            // The cutoff may separate two valid floating-point approximations. Validate
            // the worker's decision against its independently checked probability.
            boundary_reference
                .validate(&boundary_request, &boundary_digest)
                .unwrap();

            for prior in &mut reference.priors {
                prior.probability_above_threshold += 0.001;
            }
            assert!(reference.validate(&request, &request_digest).is_err());
        }
    }
}
