#![cfg(feature = "cli")]

use std::{fs, path::Path};

use assert_cmd::Command;

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

[multiscale_residual]
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
memory_budget_mib = 64
k_chunk_modes = 16
strict_repro = false
save_intermediates = false

[output]
write_parquet_curves = false
write_geojson_territories = false
write_figures = false
write_run_manifest = false
"#,
    )
    .expect("config");
}

fn analyze(cells: &Path, mask: &Path, config: &Path, out: &Path) {
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "analyze",
            "--cells",
            cells.to_str().expect("cells path"),
            "--mask",
            mask.to_str().expect("mask path"),
            "--config",
            config.to_str().expect("config path"),
            "--out",
            out.to_str().expect("output path"),
            "--threads",
            "1",
        ])
        .assert()
        .success();
}

fn project_prepost(project: &Path, pre: &Path, post: &Path, out: &Path) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    command.args([
        "project",
        "marked-prepost",
        "--project",
        project.to_str().expect("project path"),
        "--pre",
        pre.to_str().expect("pre path"),
        "--post",
        post.to_str().expect("post path"),
        "--out",
        out.to_str().expect("output path"),
    ]);
    command
}

#[test]
fn marked_prepost_replays_three_typed_nodes_without_second_execution() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let pre_cells = directory.path().join("pre.csv");
    let post_cells = directory.path().join("post.csv");
    let mask = directory.path().join("mask.geojson");
    let config = directory.path().join("config.toml");
    let pre = directory.path().join("pre-result");
    let post = directory.path().join("post-result");
    let project = directory.path().join("project");
    let first = directory.path().join("first.json");
    let second = directory.path().join("second.json");
    fs::write(
        &pre_cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0,0,1,case_01,pre,MSH6,true,true\n\
1,0,0,case_01,pre,MSH6,true,true\n\
2,0,0,case_01,pre,MSH6,true,true\n\
3,0,0,case_01,pre,MSH6,true,true\n",
    )
    .expect("pre fixture");
    fs::write(
        &post_cells,
        "x_um,y_um,mark,case_id,timepoint,protein,valid_tumor,valid_ihc\n\
0,0,1,case_01,post,MSH6,true,true\n\
1,0,0,case_01,post,MSH6,true,true\n\
2,0,1,case_01,post,MSH6,true,true\n\
3,0,0,case_01,post,MSH6,true,true\n",
    )
    .expect("post fixture");
    fs::write(
        &mask,
        r#"{"type":"MultiPolygon","coordinates":[[[[-1,-1],[4,-1],[4,1],[-1,1],[-1,-1]]]]}"#,
    )
    .expect("mask fixture");
    write_config(&config);
    analyze(&pre_cells, &mask, &config, &pre);
    analyze(&post_cells, &mask, &config, &post);

    project_prepost(&project, &pre, &post, &first)
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "pre=miss post=miss comparison=miss",
        ));
    project_prepost(&project, &pre, &post, &second)
        .assert()
        .success()
        .stderr(predicates::str::contains("pre=hit post=hit comparison=hit"));

    let first_bytes = fs::read(&first).expect("first comparison");
    assert_eq!(first_bytes, fs::read(&second).expect("replayed comparison"));
    let result: serde_json::Value = serde_json::from_slice(&first_bytes).expect("result JSON");
    assert_eq!(result["format_version"], "0.3");
    assert_eq!(result["analysis"]["kind"], "marked_prepost");

    let ledger = fs::read_to_string(project.join("executions.jsonl")).expect("ledger");
    let records = ledger.lines().collect::<Vec<_>>();
    assert_eq!(records.len(), 3, "replay must not append executions");
    let node_ids = records
        .iter()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("ledger JSON")["identity"]
                ["node"]["id"]
                .as_str()
                .expect("node ID")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        node_ids,
        ["marked-pre-import", "marked-post-import", "marked-prepost"]
    );
}
