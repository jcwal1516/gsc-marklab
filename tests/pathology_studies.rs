use serde_json::{json, Value};

fn recipe() -> Value {
    let slides = [("p1", "A", 2), ("p2", "A", 3), ("p3", "B", 8), ("p4", "B", 9)]
        .into_iter().map(|(patient, group, tumor)| {
            let cells = (0..tumor + 2).map(|i| json!({
                "id":format!("c{i:03}"),"x_um":50.0 + i as f64 * 50.0,"y_um":100.0,
                "phenotype":if i < 2 {"immune"} else {"tumor"},"stratum":"all"
            })).collect::<Vec<_>>();
            json!({"slide_id":patient,"patient_id":patient,"group":group,
                "coordinate_frame_id":format!("frame-{patient}"),
                "window":{"type":"MultiPolygon","coordinates":[[[[0,0],[1000,0],[1000,1000],[0,1000],[0,0]]]]},"cells":cells})
        }).collect::<Vec<_>>();
    json!({"format":"marklab.pathology_composition_recipe","version":1,
        "study":{"study_id":"synthetic","phenotypes":{"names":["immune","tumor"],
          "measurement_status":"measured","provenance":"synthetic hard labels"},"slides":slides},
        "design":{"group_a":"A","group_b":"B","permutations":19,"bootstrap_replicates":99,"seed":42,"alpha":0.05,
          "balances":[{"name":"immune_vs_tumor","kind":"amalgamated_log_ratio","numerator":["immune"],"denominator":["tumor"]}],
          "maup":{"phenotype":"immune","origin_um":[0,0],"primary":"base","grids":[
            {"id":"base","width_um":500,"offset_um":[0,0]},
            {"id":"shift","width_um":500,"offset_um":[250,250]}]}},
        "limits":{"maximum_cells":10000,"maximum_work":10000000,"maximum_geometry_vertices":10000,
          "maximum_tiles":10000,"memory_budget_bytes":67108864}})
}

#[test]
fn composition_separates_density_from_fraction_and_conserves_partitions() {
    let result =
        marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&recipe()).unwrap())
            .unwrap();
    let value = serde_json::to_value(result).unwrap();
    let patients = value["patients"].as_array().unwrap();
    assert_eq!(patients[0]["densities_per_mm2"][0], 2.0);
    assert_eq!(patients[3]["densities_per_mm2"][0], 2.0);
    assert!(
        patients[0]["fractions"][0].as_f64().unwrap()
            > patients[3]["fractions"][0].as_f64().unwrap()
    );
    assert!(
        (patients[3]["balances"][0]["value"].as_f64().unwrap() - (2.0_f64 / 9.0).ln()).abs()
            < 1e-12
    );
    for slide in value["slides"].as_array().unwrap() {
        for grid in slide["grids"].as_array().unwrap() {
            assert!((grid["area_mm2"].as_f64().unwrap() - 1.0).abs() < 1e-10);
            assert_eq!(grid["assigned_cells"], slide["total_cells"]);
        }
    }
}

#[test]
fn composition_pools_exposure_keeps_zeros_and_respects_holes() {
    let mut input = recipe();
    let mut extra = input["study"]["slides"][0].clone();
    extra["slide_id"] = json!("p1-extra");
    extra["coordinate_frame_id"] = json!("frame-extra");
    extra["window"] = json!({"type":"MultiPolygon","coordinates":[[[[0,0],[2000,0],[2000,1000],[0,1000],[0,0]]]]});
    input["study"]["slides"].as_array_mut().unwrap().push(extra);
    let result = serde_json::to_value(
        marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(
        (result["patients"][0]["densities_per_mm2"][0]
            .as_f64()
            .unwrap()
            - 4.0 / 3.0)
            .abs()
            < 1e-12
    );
    assert_eq!(result["patients"][0]["area_mm2"], 3.0);
    let mut input = recipe();
    for cell in input["study"]["slides"][0]["cells"].as_array_mut().unwrap() {
        cell["phenotype"] = json!("tumor");
    }
    input["study"]["slides"][0]["window"] = json!({"type":"MultiPolygon","coordinates":[[[[0,0],[1000,0],[1000,1000],[0,1000],[0,0]],[[400,400],[400,600],[600,600],[600,400],[400,400]]]]});
    let result = serde_json::to_value(
        marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(result["patients"][0]["balances"][0]["value"].is_null());
    assert_eq!(result["patients"].as_array().unwrap().len(), 4);
    assert_eq!(result["patients"][0]["densities_per_mm2"][0], 0.0);
    for grid in result["slides"][0]["grids"].as_array().unwrap() {
        assert!((grid["area_mm2"].as_f64().unwrap() - 0.96).abs() < 1e-12);
    }
    input["limits"]["maximum_tiles"] = json!(1);
    assert!(marklab::pathology::analyze_pathology_composition(
        &serde_json::to_vec(&input).unwrap()
    )
    .unwrap_err()
    .to_string()
    .contains("tile"));
    let mut edge = recipe();
    edge["study"]["slides"][0]["cells"][0]["x_um"] = json!(1000);
    edge["study"]["slides"][0]["cells"][0]["y_um"] = json!(1000);
    let closed = serde_json::to_value(
        marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&edge).unwrap())
            .unwrap(),
    )
    .unwrap();
    for grid in closed["slides"][0]["grids"].as_array().unwrap() {
        assert_eq!(grid["assigned_cells"], closed["slides"][0]["total_cells"]);
    }
}

#[test]
fn composition_balances_and_grid_offsets_have_independent_oracles() {
    let mut input = recipe();
    input["design"]["balances"][0]["kind"] = json!("geometric_balance");
    // One immune cell each side of x=500: shifting by 250 joins the pair.
    for slide in input["study"]["slides"].as_array_mut().unwrap() {
        slide["cells"][0]["x_um"] = json!(490.0);
        slide["cells"][1]["x_um"] = json!(510.0);
    }
    let result = serde_json::to_value(
        marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    let p = &result["patients"][3];
    assert!(
        (p["balances"][0]["value"].as_f64().unwrap() - (0.5_f64).sqrt() * (2.0_f64 / 9.0).ln())
            .abs()
            < 1e-12
    );
    assert!((p["density_variance"][0].as_f64().unwrap() - 4.0).abs() < 1e-12);
    assert!((p["density_variance"][1].as_f64().unwrap() - 28.0).abs() < 1e-12);
    input["study"]["slides"].as_array_mut().unwrap().reverse();
    for slide in input["study"]["slides"].as_array_mut().unwrap() {
        slide["cells"].as_array_mut().unwrap().reverse();
    }
    let reordered = serde_json::to_value(
        marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result, reordered);
}

#[test]
fn composition_cli_publishes_library_results_and_refuses_overwrite() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("recipe.json");
    std::fs::write(&source, serde_json::to_vec(&recipe()).unwrap()).unwrap();
    let output = directory.path().join("output");
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_marklab"))
            .args(["pathology", "composition", "--recipe"])
            .arg(&source)
            .arg("--out")
            .arg(&output)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let actual = std::fs::read(output.join("result.json")).unwrap();
    let expected = serde_json::to_vec_pretty(
        &marklab::pathology::analyze_pathology_composition(&serde_json::to_vec(&recipe()).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, expected);
    for name in [
        "patients.csv",
        "contrasts.csv",
        "maup.svg",
        "report.md",
        "slide-000-grids.geojson",
    ] {
        assert!(output.join(name).is_file());
    }
    assert!(!run().status.success());
}

fn spatial_recipe() -> Value {
    let mut input = recipe();
    input["format"] = json!("marklab.pathology_spatial_study_recipe");
    input["design"] = json!({"group_a":"A","group_b":"B","permutations":19,"seed":87,"alpha":0.05,
        "statistic":"l_minus_r","phenotype":null,"source":null,"target":null,
        "radii_um":[40,80],"pair_bandwidth_um":10,
        "intensity":{"model":"gaussian","bandwidth_um":200,"grid":[8,8],"minimum_intensity_per_um2":1e-12,
        "cross_fit_folds":null,"bandwidth_candidates_um":[],"null_mode":"refit"},"compartments":[]});
    for (p, slide) in input["study"]["slides"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        slide["cells"] = json!((0..40)
            .map(|i| json!({"id":format!("c{i:03}"),
           "x_um":100.0+((i*173+p*19)%800) as f64,"y_um":100.0+((i*317+p*31)%800) as f64,
           "phenotype":if i%2==0 {"immune"} else {"tumor"},"stratum":"all"}))
            .collect::<Vec<_>>());
    }
    input
}

#[test]
fn spatial_refits_preserve_observed_curves_and_actual_phenotype_identity() {
    let mut input = spatial_recipe();
    let refit = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    input["design"]["intensity"]["null_mode"] = json!("plugin");
    let plugin = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(refit["slides"][0]["values"], plugin["slides"][0]["values"]);
    // Refitting evaluates intensities of each simulated pattern, not the
    // observed fit; the two nulls are distinct computations, not a relabeling.
    assert_eq!(
        refit["slides"][0]["null_model"],
        "refitted_gridded_inhomogeneous_binomial_experimental"
    );
    assert_eq!(
        plugin["slides"][0]["null_model"],
        "fixed_gridded_inhomogeneous_binomial"
    );
    assert_ne!(refit["work_units"], plugin["work_units"]);
    assert_eq!(refit["patients"].as_array().unwrap().len(), 4);
    assert!(refit["inference"]["p_value"].as_f64().is_some());
    input["design"]["statistic"] = json!("cross_g_minus_one");
    input["design"]["radii_um"] = json!([150, 220]);
    input["design"]["pair_bandwidth_um"] = json!(60);
    input["design"]["source"] = json!("immune");
    input["design"]["target"] = json!("tumor");
    input["design"]["intensity"]["null_mode"] = json!("refit");
    let cross = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cross["slides"][0]["mark_id"], "phenotype");
    assert!(cross["slides"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .all(Value::is_number));
}

#[test]
fn spatial_selection_cross_fitting_compartments_and_unavailable_patients() {
    let mut input = spatial_recipe();
    input["design"]["intensity"]["bandwidth_candidates_um"] = json!([1e308]);
    assert!(marklab::pathology::analyze_pathology_spatial_study(
        &serde_json::to_vec(&input).unwrap()
    )
    .is_err());
    input["design"]["intensity"]["bandwidth_candidates_um"] = json!([150, 250]);
    let selected = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!([150.0, 250.0].contains(
        &selected["slides"][0]["intensity"]["bandwidth_um"]
            .as_f64()
            .unwrap()
    ));
    input["design"]["intensity"]["bandwidth_candidates_um"] = json!([]);
    input["design"]["intensity"]["cross_fit_folds"] = json!(2);
    input["design"]["statistic"] = json!("g_minus_one");
    input["design"]["radii_um"] = json!([150, 220]);
    input["design"]["pair_bandwidth_um"] = json!(60);
    let cross_fit = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&input).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(cross_fit["slides"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .all(Value::is_number));
    input["design"]["intensity"]["cross_fit_folds"] = Value::Null;
    input["design"]["intensity"]["model"] = json!("binary_compartments");
    input["design"]["intensity"]["null_mode"] = json!("conditional_compartments");
    input["design"]["statistic"] = json!("l_minus_r");
    let partitions=input["study"]["slides"].as_array_mut().unwrap().iter_mut().map(|s| {
        s["window"]=json!({"type":"MultiPolygon","coordinates":[[[[0,0],[500,0],[1000,0],[1000,1000],[500,1000],[0,1000],[0,0]]]]});
        json!({"slide_id":s["slide_id"],"negative_id":"left","positive_id":"right","provenance":"prespecified synthetic compartments",
            "negative":{"type":"MultiPolygon","coordinates":[[[[0,0],[500,0],[500,1000],[0,1000],[0,0]]]]},
            "positive":{"type":"MultiPolygon","coordinates":[[[[500,0],[1000,0],[1000,1000],[500,1000],[500,0]]]]}})
    }).collect::<Vec<_>>();
    input["design"]["compartments"] = json!(partitions);
    for statistic in ["l_minus_r", "g_minus_one"] {
        input["design"]["statistic"] = json!(statistic);
        let compartment = serde_json::to_value(
            marklab::pathology::analyze_pathology_spatial_study(
                &serde_json::to_vec(&input).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(compartment["slides"][0]["null_model"]
            .as_str()
            .unwrap()
            .contains("compartment"));
        assert!(compartment["slides"][0]["charged_work"].as_u64().unwrap() > 0);
    }
    let mut sparse = spatial_recipe();
    sparse["study"]["slides"][0]["cells"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let result = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&sparse).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["patients"].as_array().unwrap().len(), 4);
    assert!(result["patients"][0]["values"].is_null());
    assert!(result["inference"]["p_value"].is_null());
    sparse["limits"]["maximum_work"] = json!(1);
    assert!(marklab::pathology::analyze_pathology_spatial_study(
        &serde_json::to_vec(&sparse).unwrap()
    )
    .is_err());
}

#[test]
fn spatial_cli_matches_library_and_keeps_patient_units() {
    let directory = tempfile::tempdir().unwrap();
    let mut input = spatial_recipe();
    let mut extra = input["study"]["slides"][0].clone();
    extra["slide_id"] = json!("p1-extra");
    extra["coordinate_frame_id"] = json!("extra");
    extra["window"] = json!({"type":"MultiPolygon","coordinates":[[[[0,0],[2000,0],[2000,1000],[0,1000],[0,0]]]]});
    input["study"]["slides"].as_array_mut().unwrap().push(extra);
    let bytes = serde_json::to_vec(&input).unwrap();
    let native = marklab::pathology::analyze_pathology_spatial_study(&bytes).unwrap();
    let value = serde_json::to_value(&native).unwrap();
    let first = value["slides"][0]["values"][0].as_f64().unwrap();
    let second = value["slides"][1]["values"][0].as_f64().unwrap();
    assert!(
        (value["patients"][0]["values"][0].as_f64().unwrap() - (first + 2.0 * second) / 3.0).abs()
            < 1e-12
    );
    let source = directory.path().join("recipe.json");
    std::fs::write(&source, bytes).unwrap();
    let output = directory.path().join("output");
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_marklab"))
            .args(["pathology", "spatial-study", "--recipe"])
            .arg(&source)
            .arg("--out")
            .arg(&output)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        std::fs::read(output.join("result.json")).unwrap(),
        serde_json::to_vec_pretty(&native).unwrap()
    );
    assert!(output.join("curves.svg").is_file());
    let curves: Value =
        serde_json::from_slice(&std::fs::read(output.join("null-curves.json")).unwrap()).unwrap();
    assert_eq!(curves["format"], "marklab.pathology_spatial_null_curves");
    assert_eq!(curves["scale"], "l_minus_radius_um");
    assert_eq!(curves["radii_um"], value["design"]["radii_um"]);
    let exported = curves["slides"].as_array().unwrap();
    assert_eq!(exported.len(), value["slides"].as_array().unwrap().len());
    for (slide, reported) in exported.iter().zip(value["slides"].as_array().unwrap()) {
        assert_eq!(slide["slide_id"], reported["slide_id"]);
        assert_eq!(slide["observed"], reported["values"]);
        let simulated = slide["simulated"].as_array().unwrap();
        assert_eq!(simulated.len(), 19);
        let eligible = slide["jointly_eligible"].as_array().unwrap();
        for curve in simulated {
            for (entry, eligible) in curve.as_array().unwrap().iter().zip(eligible) {
                assert_eq!(entry.is_number(), eligible.as_bool().unwrap());
            }
        }
        if reported["unavailable"].is_null() {
            assert_eq!(slide["native_p_global"], reported["p_global"]);
            assert!(eligible.iter().all(|value| value.as_bool().unwrap()));
        }
    }
    assert!(!run().status.success());
}

fn scan_recipe() -> Value {
    let mut input = recipe();
    input["format"] = json!("marklab.pathology_scan_recipe");
    input["study"]["slides"].as_array_mut().unwrap().truncate(1);
    input["study"]["slides"][0]["cells"]=json!((0..6).map(|i|json!({"id":format!("c{i}"),"x_um":if i<3 {100+i}else{700+i},"y_um":500,"phenotype":if i<3 {"immune"}else{"tumor"},"stratum":"all"})).collect::<Vec<_>>());
    input["design"] = json!({"positive_phenotype":"immune","eligible_phenotypes":["immune","tumor"],"radii_um":[3,5],"permutations":1999,"seed":20261002,"alpha":0.05});
    input
}

#[test]
fn bernoulli_scan_matches_enumerated_six_cell_null_and_inclusive_ties() {
    let mut input = scan_recipe();
    for mask in 0u32..64 {
        if mask.count_ones() != 3 {
            continue;
        }
        for (i, c) in input["study"]["slides"][0]["cells"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            c["phenotype"] = json!(if mask & (1 << i) != 0 {
                "immune"
            } else {
                "tumor"
            });
        }
        let result = serde_json::to_value(
            marklab::pathology::analyze_pathology_scan(&serde_json::to_vec(&input).unwrap())
                .unwrap(),
        )
        .unwrap();
        let cases = (mask & 7).count_ones().max((mask >> 3).count_ones()) as f64;
        let logterm = |n: f64| if n == 0.0 { 0.0 } else { n * (n / 1.5).ln() };
        let oracle = 2.0 * (logterm(cases) + logterm(3.0 - cases));
        assert!((result["slides"][0]["winner"]["score"].as_f64().unwrap() - oracle).abs() < 1e-12);
        let p = result["slides"][0]["p_global"].as_f64().unwrap();
        if cases == 3.0 {
            assert!(
                (p - 0.1).abs() < 0.025,
                "Monte Carlo p={p}, exact enumeration p=2/20"
            );
        } else {
            assert_eq!(p, 1.0);
        }
    }
}

#[test]
fn scan_strata_support_holes_resource_limits_and_cli() {
    let mut input = scan_recipe();
    input["design"]["permutations"] = json!(199);
    input["study"]["slides"][0]["cells"]=json!((0..20).map(|i|json!({"id":format!("c{i:02}"),"x_um":if i<10 {100+i}else{700+i},"y_um":500,"phenotype":if i<10 {"immune"}else{"tumor"},"stratum":"all"})).collect::<Vec<_>>());
    input["design"]["radii_um"] = json!([15, 25]);
    input["study"]["slides"][0]["window"] = json!({"type":"MultiPolygon","coordinates":[[[[0,0],[1000,0],[1000,1000],[0,1000],[0,0]],[[100,505],[100,510],[110,510],[110,505],[100,505]]]]});
    let bytes = serde_json::to_vec(&input).unwrap();
    let native = marklab::pathology::analyze_pathology_scan(&bytes).unwrap();
    let result = serde_json::to_value(&native).unwrap();
    assert!(result["slides"][0]["p_global"].as_f64().unwrap() <= 0.05);
    assert!(
        result["slides"][0]["winner"]["geometry"]["coordinates"][0]
            .as_array()
            .unwrap()
            .len()
            > 1
    );
    assert!(result["slides"][0]["winner"]["strata"][0]["relative_enrichment"].is_null());
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("recipe.json");
    std::fs::write(&source, &bytes).unwrap();
    let output = dir.path().join("result");
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_marklab"))
            .args(["pathology", "scan", "--recipe"])
            .arg(&source)
            .arg("--out")
            .arg(&output)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        std::fs::read(output.join("result.json")).unwrap(),
        serde_json::to_vec_pretty(&native).unwrap()
    );
    assert!(!run().status.success());
    for c in input["study"]["slides"][0]["cells"].as_array_mut().unwrap() {
        c["stratum"] = c["phenotype"].clone();
    }
    let stratified = serde_json::to_value(
        marklab::pathology::analyze_pathology_scan(&serde_json::to_vec(&input).unwrap()).unwrap(),
    )
    .unwrap();
    assert!(stratified["slides"][0]["p_global"].is_null());
    assert!(stratified["slides"][0]["unavailable"]
        .as_str()
        .unwrap()
        .contains("within declared strata"));
    for label in ["immune", "tumor"] {
        for c in input["study"]["slides"][0]["cells"].as_array_mut().unwrap() {
            c["phenotype"] = json!(label);
            c["stratum"] = json!("all");
        }
        let constant = serde_json::to_value(
            marklab::pathology::analyze_pathology_scan(&serde_json::to_vec(&input).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(constant["slides"][0]["p_global"].is_null());
    }
    let mut bounded = scan_recipe();
    bounded["limits"]["maximum_work"] = json!(100);
    assert!(
        marklab::pathology::analyze_pathology_scan(&serde_json::to_vec(&bounded).unwrap()).is_err()
    );
}

#[test]
fn shipped_example_recipes_run_with_reduced_resampling() {
    let example = |name: &str| -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/pathology-studies")
            .join(name);
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };
    let patient_count = |value: &Value| value["patients"].as_array().map(Vec::len);

    let mut composition = example("composition.json");
    composition["design"]["permutations"] = json!(19);
    composition["design"]["bootstrap_replicates"] = json!(99);
    let composition = serde_json::to_value(
        marklab::pathology::analyze_pathology_composition(
            &serde_json::to_vec(&composition).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(patient_count(&composition), Some(16));
    assert_eq!(composition["slides"].as_array().unwrap().len(), 32);

    let mut spatial = example("spatial-study.json");
    spatial["design"]["permutations"] = json!(19);
    let spatial = serde_json::to_value(
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(&spatial).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(patient_count(&spatial), Some(16));

    let mut scan = example("scan.json");
    scan["design"]["permutations"] = json!(19);
    let scan = serde_json::to_value(
        marklab::pathology::analyze_pathology_scan(&serde_json::to_vec(&scan).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(scan["slides"].as_array().unwrap().len(), 32);
}

fn single_slide_spatial(cells: Vec<Value>, design: Value) -> Value {
    let mut input = spatial_recipe();
    let mut slide = input["study"]["slides"][0].clone();
    slide["cells"] = json!(cells);
    input["study"]["slides"] = json!([slide]);
    for (key, value) in design.as_object().unwrap() {
        input["design"][key] = value.clone();
    }
    input
}

fn spatial_slide(input: &Value) -> Value {
    let result =
        marklab::pathology::analyze_pathology_spatial_study(&serde_json::to_vec(input).unwrap())
            .unwrap();
    serde_json::to_value(result).unwrap()["slides"][0].clone()
}

fn cell_at(index: usize, x: f64, y: f64, phenotype: &str) -> Value {
    json!({"id":format!("c{index:03}"),"x_um":x,"y_um":y,"phenotype":phenotype,"stratum":"all"})
}

#[test]
fn declared_support_rule_handles_sparse_centers_and_zero_pair_curves() {
    let rule = json!({"kind":"expected_pair_count","minimum_expected_pairs":1.0,
        "maximum_draws_per_simulation":50});
    // Four sources among 36 targets: many simulated patterns have no source at least
    // r + h = 240 um from the border, which leaves that radius without eligible centers.
    let sparse = (0..40)
        .map(|i| {
            let (x, y) = if i < 4 {
                [
                    (500.0, 500.0),
                    (150.0, 820.0),
                    (870.0, 130.0),
                    (300.0, 700.0),
                ][i]
            } else {
                (
                    20.0 + ((i * 271) % 960) as f64,
                    20.0 + ((i * 613) % 960) as f64,
                )
            };
            cell_at(i, x, y, if i < 4 { "immune" } else { "tumor" })
        })
        .collect::<Vec<_>>();
    let cross = json!({"statistic":"cross_g_minus_one","phenotype":null,"source":"immune",
        "target":"tumor","radii_um":[100,200],"pair_bandwidth_um":40});
    let without = spatial_slide(&single_slide_spatial(sparse.clone(), cross.clone()));
    assert!(without["unavailable"].is_string());
    assert!(without.get("expected_pairs").is_none());
    let mut with_rule = cross.clone();
    with_rule["support_rule"] = rule.clone();
    let supported = spatial_slide(&single_slide_spatial(sparse, with_rule));
    assert!(supported["unavailable"].is_null(), "{supported}");
    assert!(supported["p_global"].as_f64().is_some());
    assert_eq!(supported["expected_pairs"].as_array().unwrap().len(), 2);
    assert!(supported["support_redraws"].as_u64().is_some());

    // Two tight clusters 566 um apart: no pairs lie 160-240 um apart, so g(200) is an estimate
    // of zero with or without a declared support rule.
    let clustered = (0..40)
        .map(|i| {
            let center = if i % 2 == 0 { 300.0 } else { 700.0 };
            let offset = |k: usize| ((k * 37) % 50) as f64 - 25.0;
            cell_at(i, center + offset(i), center + offset(i * 7 + 3), "immune")
        })
        .collect::<Vec<_>>();
    let g = json!({"statistic":"g_minus_one","phenotype":null,"source":null,"target":null,
        "radii_um":[100,200],"pair_bandwidth_um":40});
    let without = spatial_slide(&single_slide_spatial(clustered.clone(), g.clone()));
    assert!(without["unavailable"].is_null(), "{without}");
    assert_eq!(without["values"][1].as_f64(), Some(-1.0));
    assert!(without["p_global"].as_f64().is_some());
    let mut with_rule = g.clone();
    with_rule["support_rule"] = rule.clone();
    let supported = spatial_slide(&single_slide_spatial(clustered.clone(), with_rule.clone()));
    assert!(supported["unavailable"].is_null(), "{supported}");
    assert_eq!(supported["values"][1].as_f64(), Some(-1.0));
    assert!(supported["p_global"].as_f64().is_some());

    // A rule the pattern cannot meet keeps every bin and reports the slide unavailable.
    with_rule["support_rule"]["minimum_expected_pairs"] = json!(1e6);
    let strict = spatial_slide(&single_slide_spatial(clustered, with_rule));
    let reason = strict["unavailable"].as_str().unwrap();
    assert!(
        reason.contains("expected-pair support rule fails at radius 100"),
        "{reason}"
    );
    assert!(strict["p_global"].is_null());
}
