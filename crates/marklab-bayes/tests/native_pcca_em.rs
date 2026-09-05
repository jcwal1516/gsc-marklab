use marklab_bayes::{fit_pcca_em, PccaEmDesign, PccaEmModality, PccaEmRow, PccaEmSpec};
use serde_json::Value;

fn fixture() -> PccaEmSpec {
    let rows = (0..12)
        .map(|index| {
            let z = index as f64 - 5.5;
            let noise = if index % 2 == 0 { 0.08 } else { -0.08 };
            PccaEmRow {
                entity_id: format!("p{index:02}"),
                split: if index < 8 { "train" } else { "test" }.into(),
                x: vec![z + noise, 0.5 * z - noise],
                y: vec![2.0 * z - noise, -z + noise],
            }
        })
        .collect();
    PccaEmSpec {
        design: PccaEmDesign {
            entity_level: "patient".into(),
            modality_x: PccaEmModality {
                id: "morphology".into(),
                measurement_status: "measured".into(),
                likelihood: "gaussian".into(),
                feature_names: vec!["x1".into(), "x2".into()],
            },
            modality_y: PccaEmModality {
                id: "ihc".into(),
                measurement_status: "measured".into(),
                likelihood: "gaussian".into(),
                feature_names: vec!["y1".into(), "y2".into()],
            },
            missingness_assumption: "complete_paired_rows".into(),
            coordinate_frame: None,
        },
        rows,
        latent_dimensions: 1,
        regularization: 1e-6,
        noise_floor: 1e-6,
        maximum_iterations: 200,
        convergence_tolerance: 1e-9,
        timeout_seconds: 30,
    }
}

#[test]
fn native_pcca_recovers_shared_direction_and_heldout_view() {
    let mut input = fixture();
    input.rows.reverse();
    let fit = fit_pcca_em(input).expect("native pCCA");
    assert_eq!(fit.format, "marklab.probabilistic_cca");
    assert_eq!(fit.version, 2);
    assert_eq!(fit.backend.name, "marklab-rust");
    assert_eq!(fit.standardization.fit_row_count, 8);
    assert_eq!(fit.heldout_row_count, 4);
    assert!(fit.canonical_correlations[0] > 0.99);
    assert!(fit.heldout_cross_view_rmse_y_from_x < 0.25);
    assert_eq!(fit.posterior_scores.len(), 12);
    assert_eq!(fit.posterior_scores[0].entity_id, "p00");
    assert!(fit.diagnostics.monotone_violations <= 1);
}

#[test]
fn native_pcca_rejects_leakage_degeneracy_and_invalid_contracts() {
    let mut duplicate = fixture();
    duplicate.rows[1].entity_id = duplicate.rows[0].entity_id.clone();
    assert!(fit_pcca_em(duplicate).is_err());

    let mut constant_train = fixture();
    for row in constant_train
        .rows
        .iter_mut()
        .filter(|row| row.split == "train")
    {
        row.x[0] = 1.0;
    }
    assert!(fit_pcca_em(constant_train).is_err());

    let mut missing_test = fixture();
    missing_test.rows.retain(|row| row.split == "train");
    assert!(fit_pcca_em(missing_test).is_err());

    let mut malformed = fixture();
    malformed.design.modality_y.id = malformed.design.modality_x.id.clone();
    assert!(fit_pcca_em(malformed).is_err());

    let mut unbounded = fixture();
    unbounded.maximum_iterations = 10_001;
    assert!(fit_pcca_em(unbounded).is_err());

    let mut no_deadline = fixture();
    no_deadline.timeout_seconds = 0;
    assert!(fit_pcca_em(no_deadline).is_err());
}

#[test]
fn pcca_frozen_python_oracles_match_identified_outputs_and_rotation_invariants() {
    for name in ["small", "representative", "demanding"] {
        let directory = format!("{}/tests/fixtures/pcca_em", env!("CARGO_MANIFEST_DIR"));
        let spec: PccaEmSpec = serde_json::from_slice(
            &std::fs::read(format!("{directory}/{name}.spec.json")).unwrap(),
        )
        .unwrap();
        let oracle: Value = serde_json::from_slice(
            &std::fs::read(format!("{directory}/{name}.oracle.json")).unwrap(),
        )
        .unwrap();
        let native = serde_json::to_value(fit_pcca_em(spec).unwrap()).unwrap();

        assert_eq!(native["format"], oracle["format"]);
        assert_eq!(native["design"], oracle["design"]);
        assert_eq!(native["heldout_row_count"], oracle["heldout_row_count"]);
        assert_eq!(native["claim_status"], oracle["claim_status"]);
        for field in ["mean_x", "scale_x", "mean_y", "scale_y"] {
            compare_vector(
                &native["standardization"][field],
                &oracle["standardization"][field],
                2e-10,
            );
        }
        for field in ["noise_diagonal_x", "noise_diagonal_y"] {
            compare_vector(
                &native["parameters"][field],
                &oracle["parameters"][field],
                2e-5,
            );
        }
        compare_vector(
            &native["canonical_correlations"],
            &oracle["canonical_correlations"],
            2e-5,
        );
        close(
            native["heldout_cross_view_rmse_y_from_x"].as_f64().unwrap(),
            oracle["heldout_cross_view_rmse_y_from_x"].as_f64().unwrap(),
            2e-5,
        );
        assert_eq!(
            native["diagnostics"]["converged"],
            oracle["diagnostics"]["converged"]
        );
        assert_eq!(
            native["diagnostics"]["iterations"],
            oracle["diagnostics"]["iterations"]
        );
        assert_eq!(
            native["diagnostics"]["monotone_violations"],
            oracle["diagnostics"]["monotone_violations"]
        );
        compare_vector(
            &native["diagnostics"]["log_likelihood_trace"],
            &oracle["diagnostics"]["log_likelihood_trace"],
            2e-5,
        );
        compare_implied_covariance(&native, &oracle, 3e-5);
        compare_posterior_reconstruction(&native, &oracle, 3e-5);
    }
}

#[test]
fn pcca_heldout_values_do_not_change_training_fit() {
    let baseline = serde_json::to_value(fit_pcca_em(fixture()).unwrap()).unwrap();
    let mut changed = fixture();
    for row in changed.rows.iter_mut().filter(|row| row.split == "test") {
        row.x
            .iter_mut()
            .for_each(|value| *value = *value * -17.0 + 31.0);
        row.y
            .iter_mut()
            .for_each(|value| *value = *value * 23.0 - 19.0);
    }
    let changed = serde_json::to_value(fit_pcca_em(changed).unwrap()).unwrap();
    assert_eq!(baseline["standardization"], changed["standardization"]);
    assert_eq!(baseline["parameters"], changed["parameters"]);
    assert_eq!(baseline["diagnostics"], changed["diagnostics"]);
    assert_ne!(
        baseline["heldout_cross_view_rmse_y_from_x"],
        changed["heldout_cross_view_rmse_y_from_x"]
    );
}

fn compare_vector(actual: &Value, expected: &Value, tolerance: f64) {
    let actual = actual.as_array().unwrap();
    let expected = expected.as_array().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        close(
            actual.as_f64().unwrap(),
            expected.as_f64().unwrap(),
            tolerance,
        );
    }
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * (1.0 + expected.abs()),
        "{actual} differs from {expected}"
    );
}

fn matrix(value: &Value) -> Vec<Vec<f64>> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect()
        })
        .collect()
}

fn compare_implied_covariance(native: &Value, oracle: &Value, tolerance: f64) {
    let mut native_loadings = matrix(&native["parameters"]["loadings_x"]);
    native_loadings.extend(matrix(&native["parameters"]["loadings_y"]));
    let mut oracle_loadings = matrix(&oracle["parameters"]["loadings_x"]);
    oracle_loadings.extend(matrix(&oracle["parameters"]["loadings_y"]));
    let native_noise = native["parameters"]["noise_diagonal_x"]
        .as_array()
        .unwrap()
        .iter()
        .chain(native["parameters"]["noise_diagonal_y"].as_array().unwrap())
        .map(|value| value.as_f64().unwrap())
        .collect::<Vec<_>>();
    let oracle_noise = oracle["parameters"]["noise_diagonal_x"]
        .as_array()
        .unwrap()
        .iter()
        .chain(oracle["parameters"]["noise_diagonal_y"].as_array().unwrap())
        .map(|value| value.as_f64().unwrap())
        .collect::<Vec<_>>();
    for row in 0..native_loadings.len() {
        for column in 0..native_loadings.len() {
            let actual = native_loadings[row]
                .iter()
                .zip(&native_loadings[column])
                .map(|(left, right)| left * right)
                .sum::<f64>()
                + if row == column {
                    native_noise[row]
                } else {
                    0.0
                };
            let expected = oracle_loadings[row]
                .iter()
                .zip(&oracle_loadings[column])
                .map(|(left, right)| left * right)
                .sum::<f64>()
                + if row == column {
                    oracle_noise[row]
                } else {
                    0.0
                };
            close(actual, expected, tolerance);
        }
    }
}

fn compare_posterior_reconstruction(native: &Value, oracle: &Value, tolerance: f64) {
    let mut native_loadings = matrix(&native["parameters"]["loadings_x"]);
    native_loadings.extend(matrix(&native["parameters"]["loadings_y"]));
    let mut oracle_loadings = matrix(&oracle["parameters"]["loadings_x"]);
    oracle_loadings.extend(matrix(&oracle["parameters"]["loadings_y"]));
    let native_scores = native["posterior_scores"].as_array().unwrap();
    let oracle_scores = oracle["posterior_scores"].as_array().unwrap();
    assert_eq!(native_scores.len(), oracle_scores.len());
    for (native_score, oracle_score) in native_scores.iter().zip(oracle_scores) {
        assert_eq!(native_score["entity_id"], oracle_score["entity_id"]);
        assert_eq!(native_score["split"], oracle_score["split"]);
        let native_score = native_score["mean"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect::<Vec<_>>();
        let oracle_score = oracle_score["mean"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect::<Vec<_>>();
        for feature in 0..native_loadings.len() {
            let actual = native_score
                .iter()
                .zip(&native_loadings[feature])
                .map(|(score, loading)| score * loading)
                .sum::<f64>();
            let expected = oracle_score
                .iter()
                .zip(&oracle_loadings[feature])
                .map(|(score, loading)| score * loading)
                .sum::<f64>();
            close(actual, expected, tolerance);
        }
    }
}
