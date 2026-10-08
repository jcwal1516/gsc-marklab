use serde_json::{json, Value};

fn polygon(left: f64, bottom: f64, right: f64, top: f64) -> Value {
    json!({"type":"MultiPolygon","coordinates":[[[[left,bottom],[right,bottom],
        [right,top],[left,top],[left,bottom]]]]})
}

fn recipe() -> Value {
    serde_json::from_str(include_str!("../examples/pathology-maps/recipe.json")).unwrap()
}

fn analyze(input: &Value) -> Value {
    serde_json::to_value(
        marklab::analyze_pathology_maps(&serde_json::to_vec(input).unwrap()).unwrap(),
    )
    .unwrap()
}

#[test]
fn complete_pathology_maps_use_annotation_area_composition_and_scalar_moran() {
    let result = analyze(&recipe());
    let slide = &result["slides"][0];
    let band = &slide["profiles"][0]["bands"][0];
    // A 6x6 square minus its 2-um erosion (2x2): area 32, with three cells in [-2,0).
    assert!((band["area_um2"].as_f64().unwrap() - 32.0).abs() < 1e-6);
    assert_eq!(band["cell_count"], 3);
    assert!((band["marker_mean"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-12);
    assert!((band["cell_density_per_mm2"].as_f64().unwrap() - 93_750.0).abs() < 1e-5);
    let map = &slide["neighborhood_maps"][0];
    assert_eq!(map["rows"][0]["composition"], json!([1.0, 0.0]));
    assert_eq!(map["rows"][0]["state"], "dominant");
    assert_eq!(map["rows"][0]["dominant_phenotype"], "tumor");
    assert_eq!(map["rows"][1]["state"], "mixed");
    assert_eq!(map["rows"][1]["effective_diversity"], 2.0);
    let local = &slide["local_map"]["rows"];
    for (row, expected) in local.as_array().unwrap().iter().zip([1.0, 0.0, 0.0, 1.0]) {
        assert!((row["statistic"].as_f64().unwrap() - expected).abs() < 1e-12);
        assert!(row["adjusted_p_value"].as_f64().unwrap() >= row["raw_p_value"].as_f64().unwrap());
    }
}

#[test]
fn unannotated_slides_keep_maps_without_manufacturing_structure_profiles() {
    let annotated = analyze(&recipe());
    let mut input = recipe();
    input["slides"][0]["annotations"] = json!([]);
    let output = analyze(&input);
    assert_eq!(output["slides"][0]["profiles"], json!([]));
    for field in ["local_map", "neighborhood_maps"] {
        assert_eq!(output["slides"][0][field], annotated["slides"][0][field]);
    }
}

#[test]
fn missing_constant_isolated_and_boundary_cells_remain_explicit() {
    let mut input = recipe();
    input["slides"][0]["cells"][0]["marker"] = Value::Null;
    input["slides"][0]["cells"][1]["marker"] = json!(1.0);
    input["slides"][0]["cells"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"edge","x_um":0.1,"y_um":0.1,"marker":4.0,
            "phenotype_probabilities":[0.5,0.5],"stratum":"all"
        }));
    let output = analyze(&input);
    let rows = output["slides"][0]["local_map"]["rows"].as_array().unwrap();
    assert_eq!(rows[0]["status"], "missing_marker");
    assert_eq!(rows[1]["status"], "zero_variance");
    assert_eq!(rows[4]["status"], "isolated_observed_point");
    assert!(rows.iter().all(|row| row["adjusted_p_value"].is_null()));
    assert_eq!(
        output["slides"][0]["profiles"][0]["bands"][0]["missing_markers"],
        1
    );
    // At a larger physical radius the edge cell gains neighbors but its disk crosses tissue.
    input["design"]["neighborhood_radii_um"] = json!([9.0]);
    let output = analyze(&input);
    let edge = &output["slides"][0]["neighborhood_maps"][0]["rows"][4];
    assert_eq!(edge["state"], "boundary_truncated");
    assert!(edge["composition"].is_array());
    assert!(edge["dominant_phenotype"].is_null());
}

#[test]
fn tissue_holes_clip_density_and_round_buffers_report_their_approximation() {
    let mut input = recipe();
    input["slides"][0]["window"]["coordinates"][0]
        .as_array_mut()
        .unwrap()
        .push(json!([
            [2.5, 2.5],
            [2.5, 3.5],
            [3.5, 3.5],
            [3.5, 2.5],
            [2.5, 2.5]
        ]));
    let output = analyze(&input);
    let profile = &output["slides"][0]["profiles"][0];
    assert!((profile["bands"][0]["area_um2"].as_f64().unwrap() - 31.0).abs() < 1e-6);
    // Outer 2-um Euclidean buffer of a 6x6 square: 48 + 4*pi; all lies inside the window.
    assert!(
        (profile["bands"][1]["area_um2"].as_f64().unwrap() - (48.0 + 4.0 * std::f64::consts::PI))
            .abs()
            < 0.02
    );
    assert!(profile["maximum_buffer_chord_error_um"].as_f64().unwrap() < 0.001);
    assert_eq!(profile["bands"][1]["cell_density_per_mm2"], 0.0);
}

#[test]
fn canonical_order_offset_invariance_and_slide_family_are_preserved() {
    let input = recipe();
    let original = analyze(&input);
    let mut reordered = input.clone();
    reordered["slides"][0]["cells"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(original, analyze(&reordered));
    let mut shifted = input.clone();
    for cell in shifted["slides"][0]["cells"].as_array_mut().unwrap() {
        cell["marker"] = json!(1e16 + 2.0 * cell["marker"].as_f64().unwrap());
    }
    let shifted_result = analyze(&shifted);
    for (a, b) in original["slides"][0]["local_map"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .zip(
            shifted_result["slides"][0]["local_map"]["rows"]
                .as_array()
                .unwrap(),
        )
    {
        assert_eq!(a["statistic"], b["statistic"]);
        assert_eq!(a["adjusted_p_value"], b["adjusted_p_value"]);
    }
    let mut two = input;
    let mut other = two["slides"][0].clone();
    other["slide_id"] = json!("slide-2");
    other["coordinate_frame_id"] = json!("slide-2-um");
    two["slides"].as_array_mut().unwrap().push(other);
    let result = analyze(&two);
    for row in result["slides"][0]["local_map"]["rows"].as_array().unwrap() {
        assert_eq!(
            row["adjusted_p_value"].as_f64().unwrap(),
            (2.0 * row["within_slide_adjusted_p_value"].as_f64().unwrap()).min(1.0)
        );
    }
}

#[test]
fn scalar_randomization_matches_an_independently_enumerated_small_null() {
    let mut input = recipe();
    input["design"]["permutations"] = json!(9999);
    let values = [-2.0_f64, 2.0, -1.0, 1.0];
    for (cell, value) in input["slides"][0]["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(values)
    {
        cell["marker"] = json!(value);
    }
    // Population variance is 2.5. The four fixed graph rows have degrees 1,2,2,1.
    let scores = |v: [f64; 4]| {
        [
            v[0] * v[1] / 2.5,
            v[1] * (v[0] + v[2]) / 5.0,
            v[2] * (v[1] + v[3]) / 5.0,
            v[3] * v[2] / 2.5,
        ]
    };
    let observed = scores(values);
    let mut raw = [0_usize; 4];
    let mut adjusted = [0_usize; 4];
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    if [a, b, c, d]
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != 4
                    {
                        continue;
                    }
                    let candidate = scores([values[a], values[b], values[c], values[d]]);
                    let maximum = candidate.iter().copied().map(f64::abs).fold(0.0, f64::max);
                    for row in 0..4 {
                        raw[row] +=
                            usize::from(candidate[row].abs() >= observed[row].abs() - 1e-12);
                        adjusted[row] += usize::from(maximum >= observed[row].abs() - 1e-12);
                    }
                }
            }
        }
    }
    let result = analyze(&input);
    for (index, row) in result["slides"][0]["local_map"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert!((row["statistic"].as_f64().unwrap() - observed[index]).abs() < 1e-12);
        assert!((row["raw_p_value"].as_f64().unwrap() - raw[index] as f64 / 24.0).abs() < 0.025);
        assert!(
            (row["adjusted_p_value"].as_f64().unwrap() - adjusted[index] as f64 / 24.0).abs()
                < 0.025
        );
    }
    // Preserving the two adjacent pairs makes an extreme endpoint inevitable in every draw.
    for (index, cell) in input["slides"][0]["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        cell["stratum"] = json!(if index < 2 { "a" } else { "b" });
    }
    assert!(analyze(&input)["slides"][0]["local_map"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["adjusted_p_value"] == 1.0));
}

#[test]
fn planted_marker_cluster_and_outlier_receive_corrected_interpretable_labels() {
    let mut input = recipe();
    input["slides"][0]["window"] = polygon(0.0, 0.0, 100.0, 30.0);
    input["design"]["local_radius_um"] = json!(6.0);
    input["design"]["permutations"] = json!(999);
    input["slides"][0]["cells"] = Value::Array(
        (0..60)
            .map(|i| {
                let group = i / 20;
                let value = if i == 0 {
                    -10.0
                } else if group == 0 {
                    10.0
                } else {
                    0.0
                };
                json!({"id":format!("cell-{i:03}"),"x_um":10.0+30.0*group as f64+(i%5) as f64,
            "y_um":10.0+((i%20)/5) as f64,"marker":value,
            "phenotype_probabilities":[1.0,0.0],"stratum":"all"})
            })
            .collect(),
    );
    let output = analyze(&input);
    let rows = output["slides"][0]["local_map"]["rows"].as_array().unwrap();
    assert_eq!(rows[0]["classification"], "low_high_outlier");
    assert!(rows
        .iter()
        .any(|row| row["classification"] == "high_high_hotspot"));
    assert!(rows
        .iter()
        .filter(|row| row["classification"] != "not_significant")
        .all(|row| row["adjusted_p_value"].as_f64().unwrap() <= 0.05));
}

#[test]
fn malformed_measurements_and_resource_exhaustion_fail_instead_of_publishing_partial_maps() {
    let base = recipe();
    // Full-slide callers retain explicit finite work/memory ceilings; defaults stay unchanged.
    #[cfg(target_pointer_width = "64")]
    {
        let mut extended_work = base.clone();
        extended_work["limits"] = json!({
            "maximum_pair_visits":250_000_000,
            "maximum_permutation_edge_evaluations":50_000_000_000u64,
            "memory_budget_bytes":8_u64 * 1024 * 1024 * 1024
        });
        assert!(
            marklab::analyze_pathology_maps(&serde_json::to_vec(&extended_work).unwrap()).is_ok()
        );
        for (field, above) in [
            ("maximum_pair_visits", 250_000_001u64),
            ("maximum_permutation_edge_evaluations", 50_000_000_001u64),
            ("memory_budget_bytes", 8_u64 * 1024 * 1024 * 1024 + 1),
        ] {
            let mut rejected = extended_work.clone();
            rejected["limits"][field] = json!(above);
            assert!(
                marklab::analyze_pathology_maps(&serde_json::to_vec(&rejected).unwrap()).is_err()
            );
        }
    }
    for (pointer, value) in [
        (
            "/slides/0/cells/0/phenotype_probabilities",
            json!([0.9, 0.9]),
        ),
        ("/slides/0/cells/0/x_um", json!(20.0)),
        (
            "/slides/0/annotations/0/boundary_uncertainty_um",
            json!(2.0),
        ),
        ("/design/profile_edges_um", json!([0.0, 0.0])),
        (
            "/marker/measurement_status",
            json!("inferred_to_be_measured"),
        ),
    ] {
        let mut input = base.clone();
        *input.pointer_mut(pointer).unwrap() = value;
        assert!(
            marklab::analyze_pathology_maps(&serde_json::to_vec(&input).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for field in [
        "maximum_cells",
        "maximum_pair_visits",
        "maximum_permutation_edge_evaluations",
        "maximum_geometry_work",
        "maximum_geometry_vertices",
        "memory_budget_bytes",
    ] {
        let mut input = base.clone();
        input["limits"] = json!({field:1});
        assert!(
            marklab::analyze_pathology_maps(&serde_json::to_vec(&input).unwrap()).is_err(),
            "{field}"
        );
    }
}

#[cfg(feature = "cli")]
#[test]
fn cli_publishes_library_results_and_spatial_overlays_atomically() {
    use assert_cmd::Command;
    use std::fs;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("recipe.json");
    let mut input_bytes = serde_json::to_vec(&recipe()).unwrap();
    // Exercise the CLI and library input boundary above the former 16-MiB cap.
    input_bytes.resize(16 * 1024 * 1024 + 1, b' ');
    fs::write(&source, input_bytes).unwrap();
    let out = directory.path().join("output");
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["pathology", "maps", "--recipe"])
        .arg(&source)
        .arg("--out")
        .arg(&out)
        .assert()
        .success();
    let result: Value =
        serde_json::from_slice(&fs::read(out.join("result.json")).unwrap()).unwrap();
    assert_eq!(result, analyze(&recipe()));
    let overlay: Value =
        serde_json::from_slice(&fs::read(out.join("slide-000-maps.geojson")).unwrap()).unwrap();
    assert_eq!(overlay["features"].as_array().unwrap().len(), 4);
    assert_eq!(
        overlay["features"][0]["geometry"]["coordinates"],
        json!([3.0, 5.0])
    );
    let figure = fs::read_to_string(out.join("slide-000-neighborhood-00.svg")).unwrap();
    assert!(figure.contains("tumor dominant") && figure.contains("Mixed"));
    assert!(fs::read_to_string(out.join("slide-000-local.svg"))
        .unwrap()
        .contains("high high hotspot"));
    let before = fs::read(out.join("result.json")).unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args(["pathology", "maps", "--recipe"])
        .arg(&source)
        .arg("--out")
        .arg(&out)
        .assert()
        .failure();
    assert_eq!(fs::read(out.join("result.json")).unwrap(), before);
    fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    let rejected_out = directory.path().join("over-limit");
    let rejection = Command::cargo_bin("marklab")
        .unwrap()
        .args(["pathology", "maps", "--recipe"])
        .arg(&source)
        .arg("--out")
        .arg(&rejected_out)
        .assert()
        .failure();
    assert!(String::from_utf8_lossy(&rejection.get_output().stderr).contains("64 MiB"));
    assert!(!rejected_out.exists());
}
