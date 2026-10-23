use marklab_bayes::{
    BayesError, GriddedLgcpFitSpec, GriddedLgcpFitWorkerRequest, GriddedLgcpSpec,
    InhomogeneousPoissonEvent, MidpointQuadratureValue, NumpyroGriddedLgcpSbcWorkerRequest,
    NumpyroGriddedLgcpSbcWorkerResult, NumpyroHierarchySbcWorkerRequest,
    NumpyroHierarchySbcWorkerResult, NutsSamplingSpec, RectangularWindow,
};
use serde_json::{json, Value};

#[test]
fn conditional_mark_sbc_requires_the_exact_enumeration_oracle() {
    use marklab_bayes::protocol::{
        conditional_multitype_mark_sbc as single,
        replicated_conditional_multitype_mark_sbc as replicated,
    };
    use marklab_bayes::BackendContract;
    use marklab_core::backend::versions::{JAX_VERSION, NUMPYRO_VERSION, PYTHON_VERSION};

    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: "lock".into(),
        worker_sha256: "worker".into(),
    };
    let sampling = NutsSamplingSpec {
        chains: 2,
        tune_per_chain: 500,
        draws_per_chain: 1000,
        target_accept: 0.95,
        seed: 17,
    };
    let types = vec!["A".to_owned(), "B".to_owned()];
    let single_calibration = single::Calibration {
        replicates: 1,
        rank_bins: 2,
        interval_probability: 0.9,
        minimum_rank_uniformity_p_value: 0.001,
        minimum_coverage: 0.0,
        maximum_coverage: 1.0,
    };
    let replicated_calibration = replicated::Calibration {
        replicates: 1,
        rank_bins: 2,
        interval_probability: 0.9,
        minimum_rank_uniformity_p_value: 0.001,
        minimum_coverage: 0.0,
        maximum_coverage: 1.0,
    };
    for is_replicated in [false, true] {
        let (format, names, states_field, normalizer_field) = if is_replicated {
            (
                "marklab.numpyro_replicated_conditional_multitype_mark_sbc_result",
                vec![
                    "baseline_affinity:A|B",
                    "group_affinity_shift:A|B",
                    "patient_intercept_sd",
                    "patient_potential_sd",
                    "pattern_intercept_sd",
                    "pattern_potential_sd",
                ],
                "states_per_pattern",
                "zero_parameter_log_normalizer_per_pattern",
            )
        } else {
            (
                "marklab.numpyro_conditional_multitype_mark_sbc_result",
                vec![
                    "intercept:B",
                    "potential:A|B",
                    "potential:B|B",
                    "affinity:A|B",
                ],
                "state_count",
                "zero_parameter_log_normalizer",
            )
        };
        // A failed replicate is still required to report the exact generation oracle.
        let baseline = json!({
            "format": format, "version": 1, "backend": backend, "jax_version": JAX_VERSION,
            "input_sha256": "input", "request_sha256": "request", "source_request_sha256": "source",
            "fit_state": "nonconverged", "replicates": [],
            "failures": [{"replicate": 0, "reason": "diagnostics_failed"}],
            "enumeration_oracle": {states_field: 8, normalizer_field: 3.0 * 2.0_f64.ln()},
            "diagnostics": names.iter().map(|name| json!({
                "parameter": name, "rank_histogram": [0, 0], "rank_uniformity_p_value": 0.0,
                "coverage_90": 0.0, "mean_normalized_rank": 0.0,
            })).collect::<Vec<_>>(),
        });
        let validate = |value: Value| {
            if is_replicated {
                replicated::validate_result(
                    &serde_json::from_value(value).unwrap(),
                    replicated::ValidationContext {
                        backend: &backend,
                        request_sha256: "request",
                        source_request_sha256: "source",
                        input_sha256: "input",
                        types: &types,
                        points_per_pattern: 3,
                        pattern_count: 2,
                        sampling: &sampling,
                        calibration: &replicated_calibration,
                    },
                )
            } else {
                single::validate_result(
                    &serde_json::from_value(value).unwrap(),
                    single::ValidationContext {
                        backend: &backend,
                        request_sha256: "request",
                        source_request_sha256: "source",
                        type_ids: &types,
                        point_count: 3,
                        sampling: &sampling,
                        calibration: &single_calibration,
                    },
                )
            }
        };
        validate(baseline.clone()).unwrap();
        for (field, value) in [(states_field, json!(7)), (normalizer_field, json!(0.0))] {
            let mut invalid = baseline.clone();
            invalid["enumeration_oracle"][field] = value;
            assert!(
                validate(invalid).is_err(),
                "{format}: accepted incorrect {field}"
            );
        }
        let mut invalid = baseline.clone();
        invalid["failures"][0]["replicate"] = json!(1);
        assert!(validate(invalid).is_err());
        let mut invalid = baseline.clone();
        invalid["diagnostics"][0]["rank_histogram"] = json!([1, 0]);
        assert!(validate(invalid).is_err());
        let mut invalid = baseline;
        invalid["fit_state"] = json!("complete");
        assert!(validate(invalid).is_err());
    }
}

#[test]
fn sbc_results_reject_nonconverged_rows_and_require_failure_disposition() {
    let sampling = NutsSamplingSpec {
        chains: 2,
        tune_per_chain: 500,
        draws_per_chain: 1000,
        target_accept: 0.95,
        seed: 17,
    };
    let hierarchy = NumpyroHierarchySbcWorkerRequest::new(
        0.0,
        1.0,
        0.5,
        1.0,
        6,
        4,
        20,
        sampling.clone(),
        0.001,
        0.7,
        1.0,
        "a".repeat(64),
        "b".repeat(64),
        300,
    )
    .unwrap();
    let source = GriddedLgcpFitWorkerRequest::new(
        GriddedLgcpFitSpec {
            construction: GriddedLgcpSpec {
                window: RectangularWindow {
                    xmin_um: 0.0,
                    ymin_um: 0.0,
                    xmax_um: 2.0,
                    ymax_um: 2.0,
                },
                grid_x: 2,
                grid_y: 2,
                events: vec![InhomogeneousPoissonEvent {
                    event_id: "a".into(),
                    x_um: 0.25,
                    y_um: 0.25,
                    covariate: -1.5,
                    offset: 0.0,
                }],
                grid: (0..2)
                    .flat_map(|iy| {
                        (0..2).map(move |ix| MidpointQuadratureValue {
                            ix,
                            iy,
                            covariate: f64::from(iy * 2 + ix) - 1.5,
                            offset: 0.0,
                        })
                    })
                    .collect(),
                intercept_prior_mean: 0.0,
                intercept_prior_sd: 1.0,
                coefficient_prior_mean: 0.0,
                coefficient_prior_sd: 1.0,
                field_amplitude: 0.25,
                field_length_scale_um: 1.0,
                jitter: 1e-6,
            },
        },
        sampling,
        "a".repeat(64),
        "b".repeat(64),
        300,
    )
    .unwrap();
    let gridded = NumpyroGriddedLgcpSbcWorkerRequest::new(
        source,
        20,
        0.001,
        0.7,
        1.0,
        "a".repeat(64),
        "b".repeat(64),
        300,
    )
    .unwrap();

    for is_gridded in [false, true] {
        let (request, parameters, format) = if is_gridded {
            (
                serde_json::to_value(&gridded).unwrap(),
                vec!["intercept", "coefficient", "latent_cell"],
                "marklab.numpyro_gridded_lgcp_sbc_worker_result",
            )
        } else {
            (
                serde_json::to_value(&hierarchy).unwrap(),
                vec!["global_mean", "between_patient_sd"],
                "marklab.numpyro_hierarchical_sbc_worker_result",
            )
        };
        let validate = |result: Value| -> Result<(), BayesError> {
            if is_gridded {
                serde_json::from_value::<NumpyroGriddedLgcpSbcWorkerResult>(result)
                    .unwrap()
                    .validate(&gridded, "request")
            } else {
                serde_json::from_value::<NumpyroHierarchySbcWorkerResult>(result)
                    .unwrap()
                    .validate(&hierarchy, "request")
            }
        };
        // Two ranks at the midpoint of each bin: exactly uniform, all covered.
        let rows: Vec<_> = (0..20)
            .map(|replicate| {
                let mut row = json!({
                    "replicate": replicate, "r_hat": 1.0, "ess_bulk": 1000.0,
                    "ess_tail": 1000.0, "divergences": 0, "max_tree_depth_hits": 0,
                });
                if is_gridded {
                    row["minimum_ebfmi"] = json!(0.8);
                }
                for name in &parameters {
                    row[format!("true_{name}")] = json!(0.5);
                    row[format!("{name}_rank")] = json!(100 + (replicate % 10) * 200);
                    row[format!("{name}_covered")] = json!(true);
                }
                row
            })
            .collect();
        let diagnostics: serde_json::Map<_, _> = parameters
            .iter()
            .map(|name| {
                (
                    (*name).into(),
                    json!({
                        "rank_histogram": vec![2; 10], "rank_uniformity_p_value": 1.0,
                        "coverage_90": 1.0, "mean_normalized_rank": 0.5,
                    }),
                )
            })
            .collect();
        let result = json!({
            "format": format, "version": 1, "backend": request["backend"],
            "jax_version": request["jax_version"], "request_sha256": "request",
            "fit_state": "complete", "replicates": rows, "failures": [],
            "diagnostics": diagnostics,
        });
        validate(result.clone()).unwrap();
        for (field, value) in [
            ("r_hat", json!(1.02)),
            ("r_hat", json!(0.0)),
            ("ess_bulk", json!(399.0)),
            ("ess_tail", json!(399.0)),
            ("divergences", json!(1)),
            ("max_tree_depth_hits", json!(1)),
            ("minimum_ebfmi", json!(0.2)),
        ] {
            if field == "minimum_ebfmi" && !is_gridded {
                continue;
            }
            let mut invalid = result.clone();
            invalid["replicates"][0][field] = value;
            assert!(
                matches!(validate(invalid), Err(BayesError::WorkerContract(message))
                if message.contains("replicate is invalid")),
                "{format}: {field}"
            );
        }

        // Completed fits still have uniform ranks and full coverage. The other ten
        // failures alone must prevent a complete calibration.
        let mut mixed = result.clone();
        mixed["replicates"].as_array_mut().unwrap().truncate(10);
        mixed["failures"] = json!((10..20)
            .map(|replicate| json!({
                "replicate": replicate, "reason": "diagnostics_failed:r_hat=1.02",
            }))
            .collect::<Vec<_>>());
        for name in &parameters {
            mixed["diagnostics"][*name]["rank_histogram"] = json!(vec![1; 10]);
        }
        assert!(matches!(
            validate(mixed.clone()),
            Err(BayesError::WorkerContract(message))
                if message.contains("fit state is invalid")
        ));
        mixed["fit_state"] = json!("nonconverged");
        validate(mixed).unwrap();

        let mut failed = result;
        failed["replicates"] = json!([]);
        failed["failures"] = json!((0..20)
            .map(|replicate| json!({
                "replicate": replicate, "reason": "diagnostics_failed:r_hat=1.02",
            }))
            .collect::<Vec<_>>());
        for name in parameters {
            failed["diagnostics"][name] = json!({
                "rank_histogram": vec![0; 10], "rank_uniformity_p_value": 0.0,
                "coverage_90": 0.0, "mean_normalized_rank": 0.0,
            });
        }
        assert!(matches!(
            validate(failed.clone()),
            Err(BayesError::WorkerContract(_))
        ));
        failed["fit_state"] = json!("nonconverged");
        validate(failed.clone()).unwrap();
        failed["failures"][1]["replicate"] = json!(0);
        assert!(
            matches!(validate(failed), Err(BayesError::WorkerContract(message))
            if message.contains("failure disposition is invalid"))
        );
    }
}
