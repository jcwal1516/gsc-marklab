use std::{fs, path::Path};

use assert_cmd::Command;
#[cfg(feature = "parquet")]
use mmrspace::Pattern;
use serde_json::Value;

#[cfg(not(feature = "wsi"))]
#[test]
fn slide_commands_are_absent_without_wsi_feature() {
    Command::cargo_bin("mmrspace")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("inspect-slide").not())
        .stdout(predicates::str::contains("extract-region").not());
}

fn write_config(path: &Path) {
    fs::write(
        path,
        r#"
[analysis]
mark_label = "marked"
use_probabilistic_marks = false
analyze_components = "auto"

[validation]
n_min = 4
n_marked_min = 1
n_unmarked_min = 1
p_min = 0.01
p_max = 0.99
area_min_um2 = 1.0
k_shell_min = 1
largest_interpretable_scale_fraction = 0.33
valid_mask_fraction_min = 0.5

[spectrum]
k_shells = 8
low_k_shells = 2
fit_low_k_alpha = true
anisotropy_low_k_shells = 3

[periodogram]
enabled = false

[wavelet]
enabled = false
territory_detection = false
min_territory_z = 2.5

[permutation]
b = 9
seed = 123
stratified = false
strata_fields = []

[inference]
family_wise_alpha = 0.25

[performance]
threads = 1
memory_budget_mib = 512
k_chunk_modes = 16
strict_repro = false
save_intermediates = false

[output]
write_parquet_curves = false
write_geojson_territories = true
write_figures = false
write_run_manifest = true
"#,
    )
    .expect("write config");
}

#[test]
fn analyze_cli_writes_result_json_from_csv_and_geojson_mask() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
        ])
        .assert()
        .success();

    let result_path = out.join("result.json");
    let document: Value =
        serde_json::from_str(&fs::read_to_string(result_path).expect("result json"))
            .expect("valid json");
    let result = &document["analysis"]["result"];

    assert_eq!(document["format_version"], "0.2");
    assert_eq!(document["provenance"]["program"], "mmrspace");
    assert_eq!(result["case_id"], "case_001");
    assert_eq!(result["timepoint"], "post");
    assert_eq!(result["protein"], "MSH6");
    assert_eq!(result["n_cells"], 4);
    assert_eq!(result["n_marked"], 2);
    assert!(out.join("qc.json").exists());
    assert!(out.join("timings.json").exists());
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(out.join("run_manifest.json")).expect("manifest"))
            .expect("manifest json");
    assert_eq!(manifest["command"], "analyze");
    assert_eq!(
        manifest["inputs"]["cells"].as_str(),
        Some(cells.to_string_lossy().as_ref())
    );
    assert_eq!(
        manifest["inputs"]["mask"].as_str(),
        Some(mask.to_string_lossy().as_ref())
    );
    assert_eq!(
        manifest["inputs"]["config"].as_str(),
        Some(config.to_string_lossy().as_ref())
    );
    assert_eq!(manifest["execution"]["thread_count"], 1);
    assert_eq!(manifest["execution"]["permutations"], 9);
    assert_eq!(manifest["execution"]["permutation_seed"], 123);
    assert_eq!(manifest["result"]["case_id"], "case_001");
    assert_eq!(manifest["output"]["write_run_manifest"], true);
    assert!(!out.join("wavelet_territories.geojson").exists());
    assert!(!out.join("spectra.parquet").exists());
    assert!(!out.join("pair_correlation.parquet").exists());
    assert!(!out.join("scalogram.parquet").exists());
    assert!(!out.join("figures").exists());
}

#[test]
fn analyze_cli_writes_beta_binomial_diagnostic_when_enabled() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc,component_id\n\
0.0,0.0,1,case_001,post,MSH6,true,true,1\n\
1.0,0.0,1,case_001,post,MSH6,true,true,1\n\
2.0,0.0,0,case_001,post,MSH6,true,true,1\n\
10.0,0.0,1,case_001,post,MSH6,true,true,2\n\
11.0,0.0,0,case_001,post,MSH6,true,true,2\n\
12.0,0.0,0,case_001,post,MSH6,true,true,2\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[13,-1],[13,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);
    let config_text = fs::read_to_string(&config).expect("config").replace(
        "[performance]",
        "[diagnostics]\nbeta_binomial = true\ngraph_smoothing = false\n\n[performance]",
    );
    fs::write(&config, config_text).expect("rewrite config");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
        ])
        .assert()
        .success();

    let document: Value =
        serde_json::from_str(&fs::read_to_string(out.join("result.json")).expect("result json"))
            .expect("json");
    let beta_binomial = &document["analysis"]["result"]["diagnostics"]["value"]["beta_binomial"];
    assert_eq!(
        beta_binomial["diagnostic_name"],
        "beta_binomial_group_summary_v1"
    );
    assert_eq!(beta_binomial["n_cells"], 6);
    assert_eq!(beta_binomial["groups"].as_array().expect("groups").len(), 2);

    let report = fs::read_to_string(out.join("report.md")).expect("report");
    assert!(report.contains("Optional diagnostics"));
    assert!(report.contains("Beta-binomial summary"));
    assert!(!report.to_lowercase().contains("proof"));
}

#[test]
fn analyze_cli_rejects_graph_smoothing_without_multimodal_graph() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);
    let config_text = fs::read_to_string(&config).expect("config").replace(
        "[performance]",
        "[diagnostics]\nbeta_binomial = false\ngraph_smoothing = true\n\n[performance]",
    );
    fs::write(&config, config_text).expect("rewrite config");

    let output = Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).expect("stderr");
    assert!(stderr.contains("graph_smoothing diagnostic requires multimodal analyze"));
}

#[test]
fn analyze_cli_strict_repro_records_effective_single_thread_execution() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);
    let config_text = fs::read_to_string(&config).expect("config text").replace(
        "strict_repro = false\nsave_intermediates = false",
        "strict_repro = true\nsave_intermediates = false",
    );
    fs::write(&config, config_text).expect("rewrite config");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "4",
        ])
        .assert()
        .success();

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(out.join("run_manifest.json")).expect("manifest"))
            .expect("manifest json");
    assert_eq!(manifest["execution"]["requested_threads"], 4);
    assert_eq!(manifest["execution"]["thread_count"], 1);
    assert_eq!(manifest["execution"]["strict_repro"], true);
}

#[test]
fn analyze_cli_writes_requested_trace_and_timings_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");
    let trace_json = dir.path().join("trace.jsonl");
    let timings = dir.path().join("timings-copy.json");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
            "--log",
            "debug",
            "--trace-json",
            trace_json.to_str().unwrap(),
            "--timings",
            timings.to_str().unwrap(),
        ])
        .assert()
        .success();

    let trace = fs::read_to_string(&trace_json).expect("trace jsonl");
    let trace_events = trace
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("trace event json"))
        .collect::<Vec<_>>();
    assert!(trace_events
        .iter()
        .any(|event| event["log_level"] == "debug"));

    let copied_timings: Value =
        serde_json::from_str(&fs::read_to_string(&timings).expect("timings copy"))
            .expect("timings json");
    let out_timings: Value =
        serde_json::from_str(&fs::read_to_string(out.join("timings.json")).expect("out timings"))
            .expect("out timings json");
    assert_eq!(copied_timings, out_timings);

    let expected_stages = [
        "load",
        "mask_filter",
        "nearest_neighbor",
        "validate",
        "kgrid",
        "structure_factor_observed",
        "permutation_spectra",
        "periodogram",
        "wavelet",
        "inference",
        "write_outputs",
    ];
    let timing_stages = copied_timings["stages"].as_array().expect("timing stages");
    for expected in expected_stages {
        assert!(
            timing_stages
                .iter()
                .any(|stage| stage["stage_name"] == expected),
            "timings should include {expected}"
        );
        assert!(
            trace_events
                .iter()
                .any(|event| event["stage_name"] == expected),
            "trace json should include {expected}"
        );
    }
}

#[cfg(feature = "parquet")]
#[test]
fn analyze_cli_accepts_parquet_cell_input() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.parquet");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");
    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "simulate",
            "random-labeling",
            "--n",
            "4",
            "--p",
            "0.5",
            "--seed",
            "7",
            "--out",
            cells.to_str().unwrap(),
        ])
        .assert()
        .success();
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
        ])
        .assert()
        .success();

    let document: Value =
        serde_json::from_str(&fs::read_to_string(out.join("result.json")).expect("result json"))
            .expect("json");
    let result = &document["analysis"]["result"];
    assert_eq!(result["n_cells"], 4);
    assert_eq!(result["n_marked"], 2);
}

#[cfg(feature = "parquet")]
#[test]
fn analyze_cli_writes_requested_intermediate_artifacts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let out = dir.path().join("out");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);
    let mut config_text = fs::read_to_string(&config).expect("config");
    config_text = config_text.replace("save_intermediates = false", "save_intermediates = true");
    fs::write(&config, config_text).expect("rewrite config");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "analyze",
            "--cells",
            cells.to_str().unwrap(),
            "--mask",
            mask.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
        ])
        .assert()
        .success();

    let intermediates = out.join("intermediates");
    let filtered = intermediates.join("filtered_cells.parquet");
    let kgrid = intermediates.join("kgrid.parquet");
    let raster = intermediates.join("residual_raster.npy");
    assert!(filtered.exists());
    assert!(kgrid.exists());
    assert!(raster.exists());

    let filtered_pattern = Pattern::from_paths(&filtered, &mask).expect("filtered cells parquet");
    assert_eq!(filtered_pattern.len(), 4);
    assert!(fs::metadata(kgrid).expect("kgrid metadata").len() > 0);
    assert_eq!(&fs::read(raster).expect("raster npy")[0..6], b"\x93NUMPY");
}

#[test]
fn profile_plan_cli_writes_external_profiler_commands() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("profiling_plan.md");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "profile-plan",
            "--workload",
            "representative",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let text = fs::read_to_string(out).expect("profile plan");

    assert!(text.contains("samply record"));
    assert!(text.contains("cargo flamegraph"));
    assert!(text.contains("dhat-heap"));
    assert!(text.contains("cargo asm"));
    assert!(text.contains("cargo bench"));
    assert!(text.contains("cargo bench --bench random_labeling_envelope"));
    assert!(text.contains("cargo bench --bench pattern_load"));
}

#[test]
fn analyze_cli_help_exposes_documented_heap_profile_flag() {
    let output = Command::cargo_bin("mmrspace")
        .expect("bin")
        .args(["analyze", "--help"])
        .output()
        .expect("help output");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 help");
    assert!(stdout.contains("--heap-profile"));
}

#[test]
fn simulate_random_labeling_writes_reproducible_cell_table() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("random.csv");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "simulate",
            "random-labeling",
            "--n",
            "10",
            "--p",
            "0.3",
            "--seed",
            "7",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let text = fs::read_to_string(out).expect("simulation");
    let marked = text
        .lines()
        .skip(1)
        .filter(|line| line.split(',').nth(2) == Some("1"))
        .count();

    assert_eq!(text.lines().count(), 11);
    assert_eq!(marked, 3);
}

#[cfg(feature = "parquet")]
#[test]
fn simulate_random_labeling_writes_parquet_when_requested() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("random.parquet");
    let mask = dir.path().join("mask.geojson");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "simulate",
            "random-labeling",
            "--n",
            "10",
            "--p",
            "0.3",
            "--seed",
            "7",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[11,-1],[11,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask");
    let pattern = Pattern::from_paths(&out, &mask).expect("load simulated parquet");

    assert_eq!(pattern.len(), 10);
    assert_eq!(pattern.n_marked(), 3);
    assert_eq!(pattern.meta.case_id, "simulated");
}

#[test]
fn validate_synthetic_writes_machine_readable_summary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("validation_run");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "validate",
            "--suite",
            "synthetic",
            "--replicates",
            "5",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: Value =
        serde_json::from_str(&fs::read_to_string(out.join("validation.json")).expect("validation"))
            .expect("json");

    assert_eq!(result["suite"], "synthetic");
    assert_eq!(result["replicates"], 5);
    assert_eq!(result["status"], "completed");
    assert_eq!(
        result["generators"].as_array().expect("generators").len(),
        12
    );

    let results = result["results"].as_object().expect("results object");
    for generator in [
        "random_labeling",
        "single_gaussian_cluster",
        "single_matern_cluster",
        "many_small_foci",
        "anisotropic_stripe",
        "low_k_suppressed_dispersed",
        "cell_density_gradient_random_labels",
        "stain_gradient_artifact",
        "internal_control_dropout_artifact",
        "fragmented_tumor_islands",
        "rare_phenotype",
        "serial_section_misregistration",
    ] {
        assert!(results.contains_key(generator), "missing {generator}");
        assert_eq!(results[generator]["replicates_run"], 5);
        assert!(results[generator]["passed"].is_boolean());
    }

    assert!(results["random_labeling"]["type_i_error_alpha_0_05"].is_number());
    assert!(results["random_labeling"]["mean_low_k_excess"].is_number());
    assert!(results["single_gaussian_cluster"]["detection_rate"].is_number());
    assert!(results["anisotropic_stripe"]["mean_anisotropy_index"].is_number());
    assert!(results["stain_gradient_artifact"]["suppression_rate"].is_number());
    assert!(results["fragmented_tumor_islands"]["status_flags"]
        .as_array()
        .expect("fragment flags")
        .iter()
        .any(|flag| flag == "MaskFragmentationSuspect"));
}

#[test]
fn batch_cli_runs_manifest_rows_into_named_output_dirs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cells = dir.path().join("cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let manifest = dir.path().join("manifest.csv");
    let out = dir.path().join("batch");

    fs::write(
        &cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);
    fs::write(
        &manifest,
        format!(
            "id,cells,mask\ncase_001_post,{},{}\n",
            cells.display(),
            mask.display()
        ),
    )
    .expect("manifest");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "batch",
            "--manifest",
            manifest.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "1",
        ])
        .assert()
        .success();

    assert!(out.join("case_001_post").join("result.json").exists());
}

#[cfg(feature = "parallel")]
#[test]
fn batch_cli_prefers_batch_level_parallelism_for_multiple_rows() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first_cells = dir.path().join("case_001.csv");
    let second_cells = dir.path().join("case_002.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let manifest = dir.path().join("manifest.csv");
    let out = dir.path().join("batch");

    fs::write(
        &first_cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write first cells");
    fs::write(
        &second_cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_002,post,MSH6,true,true\n\
1.0,0.0,0,case_002,post,MSH6,true,true\n\
2.0,0.0,1,case_002,post,MSH6,true,true\n\
3.0,0.0,0,case_002,post,MSH6,true,true\n",
    )
    .expect("write second cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);
    fs::write(
        &manifest,
        format!(
            "id,cells,mask\ncase_001_post,{},{}\ncase_002_post,{},{}\n",
            first_cells.display(),
            mask.display(),
            second_cells.display(),
            mask.display()
        ),
    )
    .expect("manifest");

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "batch",
            "--manifest",
            manifest.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--threads",
            "2",
        ])
        .assert()
        .success();

    for id in ["case_001_post", "case_002_post"] {
        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(out.join(id).join("run_manifest.json")).expect("manifest"),
        )
        .expect("manifest json");
        assert_eq!(manifest["execution"]["requested_threads"], 1);
        assert_eq!(manifest["execution"]["thread_count"], 1);
    }
}

#[test]
fn prepost_cli_writes_delta_result_with_safe_language() {
    let dir = tempfile::tempdir().expect("temp dir");
    let pre_cells = dir.path().join("pre_cells.csv");
    let post_cells = dir.path().join("post_cells.csv");
    let mask = dir.path().join("mask.geojson");
    let config = dir.path().join("config.toml");
    let pre_out = dir.path().join("pre");
    let post_out = dir.path().join("post");
    let delta_out = dir.path().join("delta");

    fs::write(
        &pre_cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,pre,MSH6,true,true\n\
1.0,0.0,0,case_001,pre,MSH6,true,true\n\
2.0,0.0,0,case_001,pre,MSH6,true,true\n\
3.0,0.0,0,case_001,pre,MSH6,true,true\n",
    )
    .expect("write pre cells");
    fs::write(
        &post_cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0.0,0.0,1,case_001,post,MSH6,true,true\n\
1.0,0.0,0,case_001,post,MSH6,true,true\n\
2.0,0.0,1,case_001,post,MSH6,true,true\n\
3.0,0.0,0,case_001,post,MSH6,true,true\n",
    )
    .expect("write post cells");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("write mask");
    write_config(&config);

    for (cells, out) in [(&pre_cells, &pre_out), (&post_cells, &post_out)] {
        Command::cargo_bin("mmrspace")
            .expect("bin")
            .args([
                "analyze",
                "--cells",
                cells.to_str().unwrap(),
                "--mask",
                mask.to_str().unwrap(),
                "--config",
                config.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--threads",
                "1",
            ])
            .assert()
            .success();
    }

    Command::cargo_bin("mmrspace")
        .expect("bin")
        .args([
            "prepost",
            "--pre",
            pre_out.join("result.json").to_str().unwrap(),
            "--post",
            post_out.join("result.json").to_str().unwrap(),
            "--out",
            delta_out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let text = fs::read_to_string(delta_out.join("prepost.json")).expect("delta");
    let lower = text.to_lowercase();
    assert!(lower.contains("coarse-scale spatial organization"));
    assert!(!lower.contains("same cells"));
}
