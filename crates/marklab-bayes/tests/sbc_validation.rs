use marklab_bayes::{
    BayesError, GriddedLgcpFitSpec, GriddedLgcpFitWorkerRequest, GriddedLgcpSpec,
    InhomogeneousPoissonEvent, MidpointQuadratureValue, NumpyroGriddedLgcpSbcWorkerRequest,
    NumpyroGriddedLgcpSbcWorkerResult, NumpyroHierarchySbcWorkerRequest,
    NumpyroHierarchySbcWorkerResult, NutsSamplingSpec, RectangularWindow,
};
use serde_json::{json, Value};

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
