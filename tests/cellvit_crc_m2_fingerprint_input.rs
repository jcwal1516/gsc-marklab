use std::{fs, path::Path, process::Command};

const M0: &[&str] = &[
    "embedding_stage_ordinal",
    "embedding_log1p_all_cell_count",
    "embedding_tumor_fraction",
    "embedding_inflammatory_fraction",
    "embedding_connective_fraction",
    "embedding_cell_density_per_mm2",
];

fn write_json(path: &Path, value: serde_json::Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("JSON parent");
    }
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).expect("JSON fixture");
}

#[test]
fn m2_adapter_joins_classical_and_graph_fourier_results_into_heldout_folds() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let m0 = dir.path().join("m0");
    let classical = dir.path().join("classical");
    let graph2 = dir.path().join("graph2");
    let graph3 = dir.path().join("graph3");
    let patients = [("p1", "a"), ("p2", "a"), ("p3", "b"), ("p4", "b")];
    let mut admitted = String::from("patient_id,label,stage,project_id,site_id\n");
    for (index, (patient, site)) in patients.iter().enumerate() {
        admitted.push_str(&format!(
            "{patient},{},Stage II,TCGA-COAD,{site}\n",
            if index % 2 == 0 { "MSI" } else { "MSS" }
        ));
        let header = format!(
            "region_id,patient_id,site_id,domain,provenance_sha256,{}\n",
            M0.join(",")
        );
        let values = (0..M0.len())
            .map(|offset| (index + offset + 1).to_string())
            .collect::<Vec<_>>()
            .join(",");
        for policy in ["patient_held_out", "site_held_out"] {
            let fold = m0.join(policy).join(patient);
            fs::create_dir_all(&fold).expect("M0 fold");
            fs::write(
                fold.join("query.csv"),
                format!(
                    "{header}{patient},{patient},{site},m0,{},{}\n",
                    "a".repeat(64),
                    values
                ),
            )
            .expect("M0 query");
            fs::write(
                fold.join("training.csv"),
                format!(
                    "{header}{patient},{patient},{site},m0,{},{}\n",
                    "a".repeat(64),
                    values
                ),
            )
            .expect("M0 training");
        }
        write_json(
            &classical.join(patient).join("result.json"),
            serde_json::json!({
                "format":"marklab.classical_spatial",
                "analysis": {"case_id":patient,"status":"available","curve":[
                    {"radius_um":25.0,"status":"available","l":30.0},
                    {"radius_um":50.0,"status":"available","l":55.0},
                    {"radius_um":75.0,"status":"available","l":75.0},
                    {"radius_um":100.0,"status":"available","l":90.0}
                ]}
            }),
        );
        for (root, grid) in [(&graph2, 2), (&graph3, 3)] {
            write_json(
                &root.join(format!("{patient}.json")),
                serde_json::json!({
                    "format":"marklab.graph_spectral","version":1,
                    "radius_um":100.0 / grid as f64,
                    "frequency_bands":[
                        {"id":"low","energy_fraction":0.5},
                        {"id":"middle","energy_fraction":0.3},
                        {"id":"high","energy_fraction":0.2}
                    ]
                }),
            );
        }
    }
    fs::write(dir.path().join("admitted.csv"), admitted).expect("admitted fixture");
    let out = dir.path().join("out");
    let status = Command::new("python3")
        .arg("workers/python/marklab_tcga_crc_m2_fingerprint_adapter.py")
        .args([
            "--m0-folds-root",
            m0.to_str().unwrap(),
            "--admitted-patients",
            dir.path().join("admitted.csv").to_str().unwrap(),
            "--classical-results",
            classical.to_str().unwrap(),
            "--graph-2x2-results",
            graph2.to_str().unwrap(),
            "--graph-3x3-results",
            graph3.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("run M2 adapter");
    assert!(status.success());

    let admission: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("admission.json")).expect("M2 admission"))
            .expect("admission JSON");
    assert_eq!(admission["patient_count"], 4);
    assert_eq!(admission["population_unit"], "patient");
    assert_eq!(admission["feature_count"], 16);
    assert_eq!(admission["molecular_labels_used_for_features"], false);

    let query =
        fs::read_to_string(out.join("folds/m2/patient_held_out/p1/query.csv")).expect("M2 query");
    let mut rows = csv::Reader::from_reader(query.as_bytes());
    let headers = rows.headers().unwrap().clone();
    let row = rows.records().next().unwrap().unwrap();
    let value = |name: &str| {
        row[headers.iter().position(|header| header == name).unwrap()]
            .parse::<f64>()
            .unwrap()
    };
    assert!((value("embedding_coordinate_l_relative_25um") - 0.2).abs() < 1e-12);
    assert!((value("embedding_coordinate_l_relative_100um") + 0.1).abs() < 1e-12);
    assert_eq!(
        value("embedding_coordinate_graph_2x2_low_energy_fraction"),
        0.5
    );

    let training = fs::read_to_string(out.join("folds/m2/site_held_out/p1/training.csv"))
        .expect("site-held training");
    assert!(!training.contains(",a,"));
    assert!(training.contains("p3") && training.contains("p4"));
}
