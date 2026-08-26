#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn death_only_agent_process_replays_exactly_from_seed() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "window": {"xmin_um": 0.0, "ymin_um": 0.0, "xmax_um": 10.0, "ymax_um": 10.0},
            "agents": [{"agent_id": "a1", "x_um": 5.0, "y_um": 5.0, "species": "a"}],
            "species_a": {"birth_rate": 0.0, "death_rate": 1.0, "move_rate": 0.0, "switch_rate": 0.0},
            "species_b": {"birth_rate": 0.0, "death_rate": 0.0, "move_rate": 0.0, "switch_rate": 0.0},
            "competition_radius_um": 1.0,
            "competition_death_per_opposite_neighbor": 0.0,
            "birth_jitter_sd_um": 0.1,
            "move_sd_um": 0.1,
            "final_time": 100.0
        }))
        .expect("JSON"),
    )
    .expect("input");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    for output in [&first, &second] {
        Command::cargo_bin("marklab")
            .expect("binary")
            .args([
                "simulate",
                "agent-competition",
                "--input",
                input.to_str().unwrap(),
                "--seed",
                "42",
                "--maximum-events",
                "100",
                "--maximum-agents",
                "100",
                "--maximum-pair-visits",
                "1000",
                "--retain-events",
                "10",
                "--out",
                output.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    let first_bytes = fs::read(first).expect("first");
    assert_eq!(first_bytes, fs::read(second).expect("second"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("JSON");
    assert_eq!(result["format"], "marklab.agent_competition");
    assert_eq!(result["final_agents"].as_array().unwrap().len(), 0);
    assert_eq!(result["event_counts"]["death"], 1);
    assert_eq!(result["completed_events"], 1);
    assert_eq!(result["termination"], "all_agents_removed");
    assert_eq!(
        result["claim_status"],
        "experimental_agent_simulation_not_evolutionary_or_treatment_truth"
    );
}

#[test]
fn zero_rate_agent_process_stops_without_inventing_events() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("input.json");
    fs::write(
        &input,
        br#"{"window":{"xmin_um":0.0,"ymin_um":0.0,"xmax_um":10.0,"ymax_um":10.0},"agents":[{"agent_id":"b1","x_um":2.0,"y_um":3.0,"species":"b"}],"species_a":{"birth_rate":0.0,"death_rate":0.0,"move_rate":0.0,"switch_rate":0.0},"species_b":{"birth_rate":0.0,"death_rate":0.0,"move_rate":0.0,"switch_rate":0.0},"competition_radius_um":1.0,"competition_death_per_opposite_neighbor":0.0,"birth_jitter_sd_um":0.1,"move_sd_um":0.1,"final_time":10.0}"#,
    )
    .expect("input");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "simulate",
            "agent-competition",
            "--input",
            input.to_str().unwrap(),
            "--seed",
            "7",
            "--maximum-events",
            "10",
            "--maximum-agents",
            "10",
            "--maximum-pair-visits",
            "100",
            "--retain-events",
            "10",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["completed_events"], 0);
    assert_eq!(result["termination"], "no_active_rates");
    assert_eq!(result["final_agents"][0]["agent_id"], "b1");
}
