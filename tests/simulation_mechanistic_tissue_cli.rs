#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn vascular_density_interface_and_agent_modules_exchange_declared_state() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let density = vec![0.25; 9];
    let phi = (0..3)
        .flat_map(|_| (0..3).map(|x| f64::from(x) - 1.0))
        .collect::<Vec<_>>();
    let mut specification = serde_json::json!({
        "grid_x": 3,
        "grid_y": 3,
        "spacing_x_um": 1.0,
        "spacing_y_um": 1.0,
        "initial_density_row_major": density,
        "initial_phi_row_major": phi,
        "vascular": {
            "initial_concentration_row_major": vec![0.0; 9],
            "diffusion_um2_per_time_row_major": vec![0.0; 9],
            "velocity_x_um_per_time_row_major": vec![0.0; 9],
            "velocity_y_um_per_time_row_major": vec![0.0; 9],
            "flow_approximation": "caller_declared_static_velocity",
            "vessel_sources": [{
                "vessel_id": "v1", "x_um": 1.0, "y_um": 1.0,
                "source_concentration_per_time": 9.0
            }],
            "uptake_cells": []
        },
        "agents": {
            "window": {"xmin_um": 0.0, "ymin_um": 0.0, "xmax_um": 2.0, "ymax_um": 2.0},
            "initial": [{"agent_id": "a1", "x_um": 1.0, "y_um": 1.0, "species": "a"}],
            "species_a": {"birth_rate": 0.0, "death_rate": 0.0, "move_rate": 0.0, "switch_rate": 0.0},
            "species_b": {"birth_rate": 0.0, "death_rate": 0.0, "move_rate": 0.0, "switch_rate": 0.0},
            "competition_radius_um": 1.0,
            "competition_death_per_opposite_neighbor": 0.0,
            "birth_jitter_sd_um": 0.0,
            "move_sd_um": 0.0,
            "seed": 42,
            "maximum_events_per_interval": 100,
            "maximum_agents": 100,
            "maximum_pair_visits_per_interval": 1000,
            "retain_events_per_interval": 10
        },
        "coupling": {
            "oxygen_half_saturation": 1.0,
            "base_density_growth_rate_per_time": 2.1972245773362196,
            "density_carrying_capacity": 1.0,
            "density_diffusion_um2_per_time": 0.0,
            "interface_base_speed_um_per_time": 0.0,
            "interface_density_speed_weight": 2.0,
            "interface_oxygen_speed_weight": 0.0,
            "interface_curvature_weight_um2_per_time": 0.0,
            "agent_hypoxia_death_rate": 1.0
        },
        "numerics": {
            "coupled_intervals": 1,
            "interval_time": 1.0,
            "vascular_time_step": 1.0,
            "density_time_step": 0.1,
            "interface_time_step": 0.1,
            "hypoxia_threshold": 0.1,
            "maximum_cell_steps_per_module_interval": 1000,
            "maximum_reinitialization_distance_visits_per_interval": 1000
        }
    });
    fs::write(&input, serde_json::to_vec(&specification).unwrap()).unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "simulate",
            "mechanistic-tissue",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.mechanistic_tissue");
    assert!((result["intervals"][0]["mean_oxygen"].as_f64().unwrap() - 1.0).abs() < 1e-12);
    assert!(
        (result["intervals"][0]["oxygen_saturation"]
            .as_f64()
            .unwrap()
            - 0.5)
            .abs()
            < 1e-12
    );
    for row in result["final_density"].as_array().unwrap() {
        assert!((row["value"].as_f64().unwrap() - 0.5).abs() < 1e-12);
    }
    for row in result["final_interface"].as_array().unwrap() {
        let expected = row["x_um"].as_f64().unwrap() - 2.0;
        assert!((row["phi"].as_f64().unwrap() - expected).abs() < 1e-12);
    }
    assert_eq!(result["final_agents"].as_array().unwrap().len(), 0);
    assert_eq!(result["intervals"][0]["agent_events"], 1);
    assert!(
        result["coupling_diagnostics"]["maximum_vascular_mass_balance_residual"]
            .as_f64()
            .unwrap()
            .abs()
            < 1e-12
    );
    assert_eq!(
        result["observation_model"],
        "identity_latent_fields_and_agents"
    );
    assert_eq!(
        result["claim_status"],
        "experimental_coupled_simulation_not_digital_twin"
    );

    specification["numerics"]["coupled_intervals"] = serde_json::json!(2);
    fs::write(&input, serde_json::to_vec(&specification).unwrap()).unwrap();
    let second_output = directory.path().join("second-result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "simulate",
            "mechanistic-tissue",
            "--input",
            input.to_str().unwrap(),
            "--out",
            second_output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(second_output).unwrap()).unwrap();
    assert_eq!(second["intervals"].as_array().unwrap().len(), 2);
    assert_eq!(second["intervals"][1]["agent_count"], 0);
    assert_eq!(second["intervals"][1]["agent_events"], 0);
}
