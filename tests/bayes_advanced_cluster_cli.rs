#![cfg(feature = "cli")]

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use marklab_bayes::advanced::thomas_cluster::ClusterModelDocument;
use marklab_bayes::RectangularWindow;
use marklab_simulation::{simulate_thomas_process, ThomasProcessSpec};

const WINDOW_UM: f64 = 400.0;
// (kappa per um^2, mean offspring, offspring SD in um) for each simulated patient.
const TRUTH: [(f64, f64, f64); 3] = [(1.5e-4, 6.0, 10.0), (1.0e-4, 8.0, 12.0), (2.0e-4, 5.0, 9.0)];

fn prior(mean: f64, sd: f64) -> serde_json::Value {
    serde_json::json!({
        "population_mean_mean": mean, "population_mean_sd": sd, "between_pattern_sd_scale": 0.5
    })
}

fn simulated_input() -> serde_json::Value {
    let patterns = TRUTH
        .iter()
        .enumerate()
        .map(|(index, &(kappa, mu, sigma))| {
            let simulated = simulate_thomas_process(ThomasProcessSpec {
                window: RectangularWindow {
                    xmin_um: 0.0,
                    ymin_um: 0.0,
                    xmax_um: WINDOW_UM,
                    ymax_um: WINDOW_UM,
                },
                kappa_parent_per_um2: kappa,
                mu_offspring: mu,
                sigma_um: sigma,
                seed: 7_000 + index as u64,
                maximum_parents: 1_000,
                maximum_offspring: 10_000,
            })
            .expect("simulated Thomas pattern");
            let points = simulated
                .offspring
                .iter()
                .map(|point| [point.x_um, point.y_um])
                .collect::<Vec<_>>();
            serde_json::json!({
                "pattern_id": format!("pattern-{index}"), "patient_id": format!("patient-{index}"),
                "window": {"xmin_um": 0.0, "xmax_um": WINDOW_UM, "ymin_um": 0.0, "ymax_um": WINDOW_UM},
                "points": points
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "patterns": patterns,
        "dilation_um": 60.0,
        "priors": {
            "log_kappa": prior(1.5e-4_f64.ln(), 1.0),
            "log_mu": prior(6.0_f64.ln(), 1.0),
            "log_sigma": prior(10.0_f64.ln(), 0.7)
        },
        "sampler": {"chains": 2, "warmup": 300, "draws": 600,
            "birth_death_proposals_per_sweep": 20, "seed": 2026},
        "diagnostic_policy": {"maximum_r_hat": 1.05, "minimum_bulk_ess": 50.0,
            "minimum_tail_ess": 50.0, "maximum_edge_truncation_fraction": 1e-3},
        "resources": {"maximum_parents_per_pattern": 300, "maximum_points_per_pattern": 1000,
            "maximum_kernel_bytes": 67_108_864_u64, "maximum_work_units": 1_000_000_000_000_u64,
            "maximum_output_bytes": 4_194_304_u64},
        "posterior_predictive_draws": 100,
        "finite_gibbs": {"sites": 6, "edges": [[0, 1], [1, 2], [2, 3], [3, 4], [4, 5]],
            "observed_types": [0, 0, 1, 1, 0, 1], "interaction_prior_sd": 1.5,
            "proposal_sd": 0.25, "draws": 1600, "burnin": 400, "seed": 311},
        "timeout_seconds": 600
    })
}

fn run(
    subcommand: &str,
    input: &serde_json::Value,
    directory: &Path,
) -> assert_cmd::assert::Assert {
    let input_path = directory.join(format!("{subcommand}.json"));
    fs::write(&input_path, serde_json::to_vec(input).unwrap()).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            subcommand,
            "--input",
            input_path.to_str().unwrap(),
            "--out",
            directory
                .join(format!("{subcommand}-result.json"))
                .to_str()
                .unwrap(),
        ])
        .assert()
}

fn contains(quantity: &serde_json::Value, truth: f64) -> bool {
    quantity["quantile_025"].as_f64().unwrap() <= truth
        && truth <= quantity["quantile_975"].as_f64().unwrap()
}

#[test]
fn bayesian_thomas_fit_recovers_simulated_clusters_and_reports_its_convergence_state() {
    let directory = tempfile::tempdir().expect("tempdir");
    run("advanced-cluster", &simulated_input(), directory.path()).success();
    let bytes = fs::read(directory.path().join("advanced-cluster-result.json")).unwrap();
    let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result["format"], "marklab.bayesian_thomas_cluster_model");
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["model"]["parent_sampler"],
        "spatial_birth_death_move_metropolis_hastings"
    );
    assert_eq!(result["diagnostics"]["chains"], 2);
    let complete = result["fit_state"] == "complete";
    assert_eq!(
        complete,
        result["nonconvergence_reasons"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        result["claim_status"],
        if complete {
            "research_only_bayesian_thomas_cluster_posterior"
        } else {
            "diagnostic_only_nonconverged"
        }
    );
    for (row, &(kappa, mu, sigma)) in result["patterns"].as_array().unwrap().iter().zip(&TRUTH) {
        assert!(contains(&row["sigma_um"], sigma), "{row}");
        assert!(
            contains(&row["offspring_intensity_per_um2"], kappa * mu),
            "{row}"
        );
        assert!(row["maximum_edge_truncation_fraction"].as_f64().unwrap() < 1e-3);
    }
    assert_eq!(
        result["finite_binary_gibbs_exchange"]["auxiliary_draw_status"],
        "exact_finite_state_enumeration"
    );
    assert!(matches!(
        ClusterModelDocument::decode(&bytes).unwrap(),
        ClusterModelDocument::BayesianThomas(_)
    ));
}

#[test]
fn version_two_heuristic_cluster_outputs_remain_readable() {
    let legacy = fs::read("tests/fixtures/legacy/advanced_cluster_v2_result.json").unwrap();
    let ClusterModelDocument::LegacyHeuristic(result) =
        ClusterModelDocument::decode(&legacy).unwrap()
    else {
        panic!("legacy heuristic output was not recognized");
    };
    assert_eq!(result.version, 2);
    assert_eq!(result.cluster_heuristic.pattern_results.len(), 6);
    assert!(result
        .limitations
        .contains(&"kmeans_heuristic_not_a_thomas_likelihood".to_owned()));
}

#[test]
fn sampler_work_beyond_the_declared_budget_is_refused_before_the_worker_starts() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mut input = simulated_input();
    input["resources"]["maximum_work_units"] = serde_json::json!(1_000);
    run("advanced-cluster", &input, directory.path())
        .failure()
        .stderr(predicates::str::contains("maximum_work_units"));
    let mut input = simulated_input();
    input["patterns"][0]["points"][0] = serde_json::json!([WINDOW_UM + 1.0, 1.0]);
    run("advanced-cluster", &input, directory.path())
        .failure()
        .stderr(predicates::str::contains("points inside it"));
}

#[test]
fn cluster_sbc_ranks_each_hierarchy_quantity_and_states_its_calibration() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = serde_json::json!({
        "window": {"xmin_um": 0.0, "xmax_um": 300.0, "ymin_um": 0.0, "ymax_um": 300.0},
        "dilation_um": 60.0,
        "priors": {
            "log_kappa": prior(1.5e-4_f64.ln(), 0.4),
            "log_mu": prior(5.0_f64.ln(), 0.4),
            "log_sigma": prior(10.0_f64.ln(), 0.3)
        },
        "sampler": {"chains": 2, "warmup": 100, "draws": 200,
            "birth_death_proposals_per_sweep": 10, "seed": 17},
        "diagnostic_policy": {"maximum_r_hat": 1.1, "minimum_bulk_ess": 20.0,
            "minimum_tail_ess": 20.0, "maximum_edge_truncation_fraction": 1.0},
        "resources": {"maximum_parents_per_pattern": 200, "maximum_points_per_pattern": 200,
            "maximum_kernel_bytes": 67_108_864_u64, "maximum_work_units": 1_000_000_000_000_u64,
            "maximum_output_bytes": 1_048_576_u64},
        "patterns_per_replicate": 1, "replicates": 4, "minimum_points": 10,
        "maximum_points": 200, "maximum_prior_attempts": 100,
        "rank_draws": 19, "rank_bins": 4, "minimum_rank_p_value": 0.01,
        "coverage_90_bounds": [0.0, 1.0], "timeout_seconds": 600
    });
    run("advanced-cluster-sbc", &input, directory.path()).success();
    let result: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.path().join("advanced-cluster-sbc-result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(result["format"], "marklab.bayesian_thomas_cluster_sbc");
    let quantities = result["quantities"].as_array().unwrap();
    assert_eq!(quantities.len(), 9);
    for row in quantities {
        let ranks = row["rank_histogram"].as_array().unwrap();
        assert_eq!(
            ranks
                .iter()
                .map(|count| count.as_u64().unwrap())
                .sum::<u64>(),
            4
        );
    }
    let passed = result["calibration_state"] == "passed";
    assert_eq!(
        result["claim_status"],
        if passed {
            "synthetic_sbc_diagnostic_not_biological_calibration"
        } else {
            "diagnostic_only_failed_calibration"
        }
    );
}
